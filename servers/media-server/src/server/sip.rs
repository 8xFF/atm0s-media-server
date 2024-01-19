use async_std::channel::Sender;
use clap::Parser;
use cluster::{
    rpc::{
        general::{NodeInfo, ServerType},
        RpcEmitter, RpcEndpoint, RpcRequest,
    },
    Cluster, ClusterEndpoint,
};
use metrics_dashboard::build_dashboard_route;
use poem::{web::Json, Route};
use poem_openapi::OpenApiService;
use std::net::SocketAddr;

use crate::rpc::http::HttpRpcServer;

use self::rpc::{cluster::SipClusterRpc, http::SipHttpApis, RpcEvent};

use super::MediaServerContext;

mod hooks;
mod middleware;
mod rpc;
mod sip_in_session;
mod sip_out_session;
mod sip_server;

pub enum InternalControl {
    ForceClose(Sender<()>),
}

/// RTMP Media Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct SipArgs {
    /// Sip listen addr, must is a specific addr, not 0.0.0.0
    #[arg(env, long)]
    pub addr: SocketAddr,

    /// Max conn
    #[arg(env, long, default_value_t = 100)]
    pub max_conn: u64,

    /// Hook url
    #[arg(env, long, default_value = "http://localhost:3000/hooks")]
    pub hook_url: String,
}

pub async fn run_sip_server<C, CR, RPC, REQ, EMITTER>(
    http_port: u16,
    http_tls: bool,
    opts: SipArgs,
    ctx: MediaServerContext<InternalControl>,
    cluster: C,
    rpc_endpoint: RPC,
) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let rpc_endpoint = SipClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port, http_tls);

    let api_service = OpenApiService::new(SipHttpApis, "Sip Server", "1.0.0").server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let node_info = NodeInfo {
        node_id: cluster.node_id(),
        address: format!("{}", cluster.node_addr()),
        server_type: ServerType::SIP,
    };

    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/node-info/", poem::endpoint::make_sync(move |_| Json(node_info.clone())))
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()));

    // Init media-server related metrics
    ctx.init_metrics();

    http_server.start(route, ctx.clone()).await;

    let hook_sender = hooks::HooksSender::new(&opts.hook_url);
    sip_server::start_server(cluster, ctx, opts.addr, hook_sender, http_server, rpc_endpoint).await;
    Ok(())
}
