use std::{sync::Arc, time::Duration};

use async_std::stream::StreamExt;
use clap::Parser;
use cluster::{
    rpc::{
        gateway::parse_conn_id,
        general::{MediaEndpointCloseRequest, MediaEndpointCloseResponse},
        webrtc::{WebrtcPatchRequest, WebrtcPatchResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse},
        RpcEmitter, RpcEndpoint, RpcRequest, RPC_MEDIA_ENDPOINT_CLOSE, RPC_WEBRTC_CONNECT, RPC_WEBRTC_ICE, RPC_WEBRTC_PATCH, RPC_WHEP_CONNECT, RPC_WHIP_CONNECT,
    },
    Cluster, ClusterEndpoint, MEDIA_SERVER_SERVICE,
};
use futures::{select, FutureExt};
use media_utils::{SystemTimer, Timer};
use metrics::describe_counter;
use metrics_dashboard::build_dashboard_route;
use poem::Route;
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

mod logic;
mod rpc;
mod webrtc_route;

const GATEWAY_SESSIONS_CONNECT_COUNT: &str = "gateway.sessions.connect.count";
const GATEWAY_SESSIONS_CONNECT_ERROR: &str = "gateway.sessions.connect.error";

/// Media Server Webrtc
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct GatewayArgs {}

pub async fn run_gateway_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, _opts: GatewayArgs, cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + Sync + 'static,
    EMITTER: RpcEmitter + Send + Sync + 'static,
{
    let node_id = cluster.node_id();
    let mut rpc_endpoint = GatewayClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port);

    let timer = Arc::new(SystemTimer());
    let api_service = OpenApiService::new(GatewayHttpApis, "Gateway Server", "1.0.0").server("http://localhost:3000");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    #[cfg(feature = "embed-samples")]
    let samples = EmbeddedFilesEndpoint::<Files>::new(Some("index.html".to_string()));
    #[cfg(not(feature = "embed-samples"))]
    let samples = StaticFilesEndpoint::new("./servers/media-server/public/").index_file("index.html");
    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
        .nest("/samples", samples);

    describe_counter!(GATEWAY_SESSIONS_CONNECT_COUNT, "Gateway sessions connect count");
    describe_counter!(GATEWAY_SESSIONS_CONNECT_ERROR, "Gateway sessions connect error count");

    http_server.start(route).await;
    let mut tick = async_std::stream::interval(Duration::from_millis(100));
    let mut gateway_logic = GatewayLogic::new();
    let rpc_emitter = rpc_endpoint.emitter();

    loop {
        let rpc = select! {
            _ = tick.next().fuse() => {
                gateway_logic.on_tick(timer.now_ms());
                continue;
            }
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
            RpcEvent::WhipConnect(req) => {
                log::info!("[Gateway] whip connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                webrtc_route::route_to_node(
                    rpc_emitter.clone(),
                    timer.clone(),
                    &mut gateway_logic,
                    node_id,
                    ServiceType::Webrtc,
                    RPC_WHIP_CONNECT,
                    &req.param().ip_addr.clone(),
                    &None,
                    &req.param().user_agent.clone(),
                    req.param().session_uuid,
                    req,
                );
            }
            RpcEvent::WhepConnect(req) => {
                log::info!("[Gateway] whep connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                webrtc_route::route_to_node(
                    rpc_emitter.clone(),
                    timer.clone(),
                    &mut gateway_logic,
                    node_id,
                    ServiceType::Webrtc,
                    RPC_WHEP_CONNECT,
                    &req.param().ip_addr.clone(),
                    &None,
                    &req.param().user_agent.clone(),
                    req.param().session_uuid,
                    req,
                );
            }
            RpcEvent::WebrtcConnect(req) => {
                log::info!("[Gateway] webrtc connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                webrtc_route::route_to_node(
                    rpc_emitter.clone(),
                    timer.clone(),
                    &mut gateway_logic,
                    node_id,
                    ServiceType::Webrtc,
                    RPC_WEBRTC_CONNECT,
                    &req.param().ip_addr.clone().expect(""),
                    &req.param().version.clone(),
                    &req.param().user_agent.clone().expect(""),
                    req.param().session_uuid.expect(""),
                    req,
                );
            }
            RpcEvent::WebrtcRemoteIce(req) => {
                if let Some((node_id, _)) = parse_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_WEBRTC_ICE, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
            RpcEvent::WebrtcSdpPatch(req) => {
                if let Some((node_id, _)) = parse_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WebrtcPatchRequest, WebrtcPatchResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_WEBRTC_PATCH, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
            RpcEvent::MediaEndpointClose(req) => {
                if let Some((node_id, _)) = parse_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<MediaEndpointCloseRequest, MediaEndpointCloseResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_MEDIA_ENDPOINT_CLOSE, req.param().clone(), 5000)
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
