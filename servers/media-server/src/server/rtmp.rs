use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use async_std::{channel::Sender, prelude::FutureExt as _};
use clap::Parser;
use cluster::{
    rpc::{
        gateway::{NodeHealthcheckResponse, NodePing, NodePong, ServiceInfo},
        general::MediaEndpointCloseResponse,
        RpcEmitter, RpcEndpoint, RpcRequest, RPC_NODE_PING,
    },
    Cluster, ClusterEndpoint, INNER_GATEWAY_SERVICE,
};
use futures::{select, FutureExt};
use media_utils::{AutoCancelTask, ErrorDebugger};
use metrics_dashboard::build_dashboard_route;
use poem::Route;
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
    #[arg(env, long)]
    port: u16,
}

pub async fn run_rtmp_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, opts: RtmpArgs, ctx: MediaServerContext<InternalControl>, mut cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let mut rtmp_tcp_server = server_tcp::RtmpServer::new(opts.port).await;
    let mut rpc_endpoint = RtmpClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port);

    let api_service = OpenApiService::new(RtmpHttpApis, "Rtmp Server", "1.0.0").server("http://localhost:3000");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    #[cfg(feature = "embed-samples")]
    let samples = EmbeddedFilesEndpoint::<Files>::new(Some("index.html".to_string()));
    #[cfg(not(feature = "embed-samples"))]
    let samples = StaticFilesEndpoint::new("./servers/media/public/rtmp").show_files_listing().index_file("index.html");
    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
        .nest("/samples", samples);

    // Init media-server related metrics
    ctx.init_metrics();

    http_server.start(route).await;

    let rtmp_port = opts.port;
    let node_id = cluster.node_id();
    let rpc_emitter = rpc_endpoint.emitter();
    let ctx_c = ctx.clone();
    let _ping_task: AutoCancelTask<_> = async_std::task::spawn_local(async move {
        async_std::task::sleep(Duration::from_secs(10)).await;
        loop {
            if let Err(e) = rpc_emitter
                .request::<_, NodePong>(
                    INNER_GATEWAY_SERVICE,
                    None,
                    RPC_NODE_PING,
                    NodePing {
                        node_id,
                        rtmp: None,
                        sip: Some(ServiceInfo {
                            usage: ((ctx_c.conns_live() * 100) / ctx_c.conns_max()) as u8,
                            live: ctx_c.conns_live() as u32,
                            max: ctx_c.conns_max() as u32,
                            addr: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), rtmp_port)),
                            domain: None,
                        }),
                        webrtc: None,
                        token: "demo-token".to_string(), //TODO implement real-token
                    },
                    5000,
                )
                .await
            {
                log::error!("[RtmpMediaServer] ping gateway error {:?}", e);
            } else {
                log::info!("[RtmpMediaServer] ping gateway success");
            }
            async_std::task::sleep(Duration::from_secs(1)).await;
        }
    })
    .into();

    loop {
        let rpc = select! {
            rpc = http_server.recv().fuse() => {
                rpc.ok_or("HTTP_SERVER_ERROR")?
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc.ok_or("CLUSTER_RPC_ERROR")?
            },
            conn = rtmp_tcp_server.recv().fuse() => {
                if let Some((room, peer, conn)) = conn {
                    match run_rtmp_endpoint(
                        ctx.clone(),
                        &mut cluster,
                        &room,
                        &peer,
                        conn,
                    )
                    .await
                    {
                        Ok(conn_id) => {
                            //TODO send conn_id to hook
                        }
                        Err(err) => {
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
            RpcEvent::NodeHeathcheck(req) => {
                req.answer(Ok(NodeHealthcheckResponse { success: true }));
            }
            RpcEvent::MediaEndpointClose(req) => {
                if let Some(old_tx) = ctx.close_conn(&req.param().conn_id) {
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
