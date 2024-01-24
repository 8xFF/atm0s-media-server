use std::{sync::Arc, time::Duration};

use async_std::stream::StreamExt;
use clap::Parser;
use cluster::{
    implement::NodeId,
    rpc::{
        gateway::{NodePing, NodePong, QueryBestNodesResponse},
        general::{MediaEndpointCloseRequest, MediaEndpointCloseResponse, MediaSessionProtocol, NodeInfo, ServerType},
        webrtc::{WebrtcPatchRequest, WebrtcPatchResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse},
        RpcEmitter, RpcEndpoint, RpcRequest, RPC_MEDIA_ENDPOINT_CLOSE, RPC_NODE_PING, RPC_WEBRTC_CONNECT, RPC_WEBRTC_ICE, RPC_WEBRTC_PATCH, RPC_WHEP_CONNECT, RPC_WHIP_CONNECT,
    },
    Cluster, ClusterEndpoint, GLOBAL_GATEWAY_SERVICE, INNER_GATEWAY_SERVICE, MEDIA_SERVER_SERVICE,
};
use futures::{select, FutureExt};
use media_utils::{SystemTimer, Timer, F32};
use metrics::describe_counter;
use metrics_dashboard::build_dashboard_route;
use poem::{web::Json, Route};
use poem_openapi::OpenApiService;

use crate::rpc::http::HttpRpcServer;

#[cfg(feature = "embed-samples")]
use crate::rpc::http::EmbeddedFilesEndpoint;
#[cfg(feature = "embed-samples")]
use rust_embed::RustEmbed;

#[cfg(not(feature = "embed-samples"))]
use poem::endpoint::StaticFilesEndpoint;

#[cfg(feature = "embed-samples")]
#[derive(RustEmbed)]
#[folder = "public"]
pub struct Files;

use self::{
    logic::{GatewayLogic, ServiceType},
    rpc::{cluster::GatewayClusterRpc, http::GatewayHttpApis, RpcEvent},
};

pub use self::logic::GatewayMode;
use super::MediaServerContext;

mod ip2location;
mod logic;
mod rpc;
mod webrtc_route;

const GATEWAY_SESSIONS_CONNECT_COUNT: &str = "gateway.sessions.connect.count";
const GATEWAY_SESSIONS_CONNECT_ERROR: &str = "gateway.sessions.connect.error";

/// Media Server Webrtc
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct GatewayArgs {
    /// Gateway mode
    #[arg(value_enum, env, long, default_value_t = GatewayMode::Inner)]
    pub mode: GatewayMode,

    /// Gateway group, only set if mode is Inner
    #[arg(env, long, default_value = "")]
    pub group: String,

    /// lat location
    #[arg(env, long, default_value_t = 0.0)]
    pub lat: f32,

    /// lng location
    #[arg(env, long, default_value_t = 0.0)]
    pub lng: f32,

    /// maxmind geo-ip db file
    #[arg(env, long, default_value = "./maxminddb-data/GeoLite2-City.mmdb")]
    pub geoip_db: String,
}

pub async fn run_gateway_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, http_tls: bool, opts: GatewayArgs, ctx: MediaServerContext<()>, cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + Sync + 'static,
    EMITTER: RpcEmitter + Send + Sync + 'static,
{
    let node_id = cluster.node_id();
    let mut rpc_endpoint = GatewayClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port, http_tls);
    let ip2location = match opts.mode {
        GatewayMode::Global => Some(ip2location::Ip2Location::new(&opts.geoip_db)),
        GatewayMode::Inner => None,
    };

    let timer = Arc::new(SystemTimer());
    let api_service = OpenApiService::new(GatewayHttpApis, "Gateway Server", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let node_info = NodeInfo {
        node_id: cluster.node_id(),
        address: format!("{}", cluster.node_addr()),
        server_type: ServerType::GATEWAY,
    };
    #[cfg(feature = "embed-samples")]
    let samples = EmbeddedFilesEndpoint::<Files>::new(Some("index.html".to_string()));
    #[cfg(not(feature = "embed-samples"))]
    let samples = StaticFilesEndpoint::new("./servers/media-server/public/").index_file("index.html");
    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/node-info/", poem::endpoint::make_sync(move |_| Json(node_info.clone())))
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
        .nest("/samples", samples);

    describe_counter!(GATEWAY_SESSIONS_CONNECT_COUNT, "Gateway sessions connect count");
    describe_counter!(GATEWAY_SESSIONS_CONNECT_ERROR, "Gateway sessions connect error count");

    http_server.start(route, ctx.clone()).await;
    let mut tick = async_std::stream::interval(Duration::from_millis(100));
    let mut gateway_logic = GatewayLogic::new(opts.mode);
    let rpc_emitter = rpc_endpoint.emitter();
    let mut gateway_feedback_tick = async_std::stream::interval(Duration::from_millis(2000));
    let dest_service_id = match opts.mode {
        GatewayMode::Global => INNER_GATEWAY_SERVICE,
        GatewayMode::Inner => MEDIA_SERVER_SERVICE,
    };

    loop {
        let rpc = select! {
            _ = tick.next().fuse() => {
                gateway_logic.on_tick(timer.now_ms());
                continue;
            }
            _ = gateway_feedback_tick.next().fuse() => {
                if matches!(opts.mode, GatewayMode::Inner) {
                    ping_global_gateway(&gateway_logic, &opts.group, (F32::<2>::new(opts.lat), F32::<2>::new(opts.lng)), node_id, &rpc_emitter);
                }

                continue;
            },
            rpc = http_server.recv().fuse() => {
                rpc.ok_or("HTTP_SERVER_ERROR")?
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc.ok_or("CLUSTER_RPC_ERROR")?
            }
        };

        match rpc {
            RpcEvent::NodePing(req) => {
                log::info!("[Gateway] node ping {:?}", req.param());
                req.answer(Ok(gateway_logic.on_ping(timer.now_ms(), req.param())));
            }
            RpcEvent::BestNodest(req) => {
                log::info!("[Gateway] best nodes {:?}", req.param());
                let nodes = gateway_logic.best_nodes(
                    ip2location.as_ref().map(|f| f.get_location(&req.param().ip_addr)).flatten(),
                    match req.param().protocol {
                        MediaSessionProtocol::Rtmp => ServiceType::Rtmp,
                        MediaSessionProtocol::Sip => ServiceType::Sip,
                        MediaSessionProtocol::Webrtc => ServiceType::Webrtc,
                        MediaSessionProtocol::Whip => ServiceType::Webrtc,
                        MediaSessionProtocol::Whep => ServiceType::Webrtc,
                    },
                    60,
                    80,
                    req.param().size,
                );
                req.answer(Ok(QueryBestNodesResponse { nodes }));
            }
            RpcEvent::WhipConnect(req) => {
                log::info!("[Gateway] whip connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                let location = ip2location.as_ref().map(|f| f.get_location(&req.param().ip_addr)).flatten();
                webrtc_route::route_to_node(
                    rpc_emitter.clone(),
                    timer.clone(),
                    &mut gateway_logic,
                    node_id,
                    ServiceType::Webrtc,
                    RPC_WHIP_CONNECT,
                    req.param().ip_addr,
                    location,
                    &None,
                    &req.param().user_agent.clone(),
                    req.param().session_uuid,
                    req,
                    dest_service_id,
                );
            }
            RpcEvent::WhepConnect(req) => {
                log::info!("[Gateway] whep connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                let location = ip2location.as_ref().map(|f| f.get_location(&req.param().ip_addr)).flatten();
                webrtc_route::route_to_node(
                    rpc_emitter.clone(),
                    timer.clone(),
                    &mut gateway_logic,
                    node_id,
                    ServiceType::Webrtc,
                    RPC_WHEP_CONNECT,
                    req.param().ip_addr,
                    location,
                    &None,
                    &req.param().user_agent.clone(),
                    req.param().session_uuid,
                    req,
                    dest_service_id,
                );
            }
            RpcEvent::WebrtcConnect(req) => {
                log::info!("[Gateway] webrtc connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                let location = ip2location.as_ref().map(|f| f.get_location(&req.param().ip_addr)).flatten();
                webrtc_route::route_to_node(
                    rpc_emitter.clone(),
                    timer.clone(),
                    &mut gateway_logic,
                    node_id,
                    ServiceType::Webrtc,
                    RPC_WEBRTC_CONNECT,
                    req.param().ip_addr,
                    location,
                    &req.param().version.clone(),
                    &req.param().user_agent.clone(),
                    req.param().session_uuid.expect("Should assign session_uuid on gateway"),
                    req,
                    dest_service_id,
                );
            }
            RpcEvent::WebrtcRemoteIce(req) => {
                if let Some(conn_id) = ctx.verifier().verify_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>(MEDIA_SERVER_SERVICE, Some(conn_id.node_id), RPC_WEBRTC_ICE, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
            RpcEvent::WebrtcSdpPatch(req) => {
                if let Some(conn_id) = ctx.verifier().verify_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WebrtcPatchRequest, WebrtcPatchResponse>(MEDIA_SERVER_SERVICE, Some(conn_id.node_id), RPC_WEBRTC_PATCH, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
            RpcEvent::MediaEndpointClose(req) => {
                if let Some(conn_id) = ctx.verifier().verify_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<MediaEndpointCloseRequest, MediaEndpointCloseResponse>(MEDIA_SERVER_SERVICE, Some(conn_id.node_id), RPC_MEDIA_ENDPOINT_CLOSE, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
        }
    }
}

fn ping_global_gateway<EMITTER: RpcEmitter + Send + 'static>(logic: &GatewayLogic, group: &str, location: (F32<2>, F32<2>), node_id: NodeId, rpc_emitter: &EMITTER) {
    let stats = logic.stats();
    let req = NodePing {
        node_id,
        group: group.to_string(),
        location: Some(location),
        rtmp: stats.rtmp,
        sip: stats.sip,
        webrtc: stats.webrtc,
    };

    let rpc_emitter = rpc_emitter.clone();
    async_std::task::spawn(async move {
        if let Err(e) = rpc_emitter.request::<_, NodePong>(GLOBAL_GATEWAY_SERVICE, None, RPC_NODE_PING, req, 1000).await {
            log::error!("[Gateway] ping global gateway error {:?}", e);
        }
    });
}
