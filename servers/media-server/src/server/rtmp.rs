use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use async_std::{channel::Sender, prelude::FutureExt as _, stream::StreamExt};
use clap::Parser;
use cluster::{
    implement::NodeId,
    rpc::{
        gateway::{NodePing, NodePong, ServiceInfo},
        general::{MediaEndpointCloseResponse, MediaSessionProtocol, NodeInfo, ServerType},
        RpcEmitter, RpcEndpoint, RpcRequest, RPC_NODE_PING,
    },
    Cluster, ClusterEndpoint, GATEWAY_SERVICE,
};
use futures::{select, FutureExt};
use media_utils::ErrorDebugger;
use metrics_dashboard::{build_dashboard_route, DashboardOptions};
use poem::{web::Json, Route};
use poem_openapi::OpenApiService;

use crate::rpc::http::HttpRpcServer;

use self::rpc::{cluster::RtmpClusterRpc, http::RtmpHttpApis, RpcEvent};

use super::MediaServerContext;
use session::run_rtmp_endpoint;

#[cfg(feature = "embed-samples")]
use crate::rpc::http::EmbeddedFilesEndpoint;
#[cfg(feature = "embed-samples")]
use rust_embed::RustEmbed;

#[cfg(not(feature = "embed-samples"))]
use poem::endpoint::StaticFilesEndpoint;

#[cfg(feature = "embed-samples")]
#[derive(RustEmbed)]
#[folder = "public/rtmp"]
pub struct Files;

mod rpc;
mod server_tcp;
mod session;

pub enum InternalControl {
    ForceClose(Sender<()>),
}

/// RTMP Media Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct RtmpArgs {
    /// Rtmp port
    #[arg(env, long, default_value_t = 1935)]
    pub port: u16,

    /// Max conn
    #[arg(env, long, default_value_t = 10)]
    pub max_conn: u64,
}

pub async fn run_rtmp_server<C, CR, RPC, REQ, EMITTER>(
    http_port: u16,
    http_tls: bool,
    zone: &str,
    opts: RtmpArgs,
    ctx: MediaServerContext<InternalControl>,
    mut cluster: C,
    rpc_endpoint: RPC,
) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let mut rtmp_tcp_server = server_tcp::RtmpServer::new(opts.port).await;
    let mut rpc_endpoint = RtmpClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port, http_tls);

    let api_service = OpenApiService::new(RtmpHttpApis, "Rtmp Server", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let node_info = NodeInfo {
        node_id: cluster.node_id(),
        address: format!("{}", cluster.node_addr()),
        server_type: ServerType::RTMP,
    };

    #[cfg(feature = "embed-samples")]
    let samples = EmbeddedFilesEndpoint::<Files>::new(Some("index.html".to_string()));
    #[cfg(not(feature = "embed-samples"))]
    let samples = StaticFilesEndpoint::new("./servers/media/public/rtmp").show_files_listing().index_file("index.html");

    let dashboard_opts = DashboardOptions {
        custom_charts: vec![],
        include_default: true,
    };
    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route(dashboard_opts))
        .nest("/ui/", ui)
        .at("/node-info/", poem::endpoint::make_sync(move |_| Json(node_info.clone())))
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
        .nest("/samples", samples);

    // Init media-server related metrics
    ctx.init_metrics();

    http_server.start(route, ctx.clone()).await;

    let rtmp_port = opts.port;
    let node_id = cluster.node_id();
    let rpc_emitter = rpc_endpoint.emitter();
    let mut gateway_feedback_tick = async_std::stream::interval(Duration::from_millis(2000));

    loop {
        let rpc = select! {
            _ = gateway_feedback_tick.next().fuse() => {
                ping_gateway(&ctx, node_id, zone, rtmp_port, &rpc_emitter);
                continue;
            },
            rpc = http_server.recv().fuse() => {
                rpc.ok_or("HTTP_SERVER_ERROR")?
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc.ok_or("CLUSTER_RPC_ERROR")?
            },
            conn = rtmp_tcp_server.recv().fuse() => {
                if let Some((token, conn)) = conn {
                    let s_token = if let Some(token) = ctx.verifier().verify_media_session(&token) {
                        if token.protocol != MediaSessionProtocol::Rtmp {
                            continue;
                        }
                        if token.peer.is_none() {
                            continue;
                        }
                        token
                    } else {
                        continue;
                    };

                    log::info!("[RtmpMediaServer] new rtmp connection from {:?}/{:?}", s_token.room, s_token.peer);

                    match run_rtmp_endpoint(
                        ctx.clone(),
                        &mut cluster,
                        &s_token.room.expect("Should have room"),
                        &s_token.peer.expect("Should have peer"),
                        conn,
                    )
                    .await
                    {
                        Ok(_conn_id) => {
                            //TODO send conn_id to hook
                        }
                        Err(_err) => {
                            //TODO send err to hook
                        }
                    }
                    continue;
                } else {
                    return Err("RTMP_SERVER_ERROR");
                }
            }
        };

        match rpc {
            RpcEvent::MediaEndpointClose(req) => {
                if let Some(old_tx) = ctx.get_conn(&req.param().conn_id) {
                    async_std::task::spawn(async move {
                        let (tx, rx) = async_std::channel::bounded(1);
                        old_tx.send(InternalControl::ForceClose(tx.clone())).await.log_error("need send");
                        if let Ok(e) = rx.recv().timeout(Duration::from_secs(1)).await {
                            let control_res = e.map_err(|_e| "INTERNAL_QUEUE_ERROR");
                            req.answer(control_res.map(|_| MediaEndpointCloseResponse { success: true }));
                        } else {
                            req.answer(Err("REQUEST_TIMEOUT"));
                        }
                    });
                } else {
                    req.answer(Err("NOT_FOUND"));
                }
            }
        }
    }
}

fn ping_gateway<EMITTER: RpcEmitter + Send + 'static>(ctx: &MediaServerContext<InternalControl>, node_id: NodeId, zone: &str, rtmp_port: u16, rpc_emitter: &EMITTER) {
    let req = NodePing {
        node_id,
        zone: zone.to_string(),
        location: None,
        rtmp: Some(ServiceInfo {
            usage: ((ctx.conns_live() * 100) / ctx.conns_max()) as u8,
            live: ctx.conns_live() as u32,
            max: ctx.conns_max() as u32,
            addr: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), rtmp_port)),
            domain: None,
        }),
        sip: None,
        webrtc: None,
    };

    let rpc_emitter = rpc_emitter.clone();
    async_std::task::spawn(async move {
        if let Err(e) = rpc_emitter.request::<_, NodePong>(GATEWAY_SERVICE, None, RPC_NODE_PING, req, 1000).await {
            log::error!("[RtmpServer] ping gateway error {:?}", e);
        }
    });
}
