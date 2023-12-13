use clap::{Parser, Subcommand};

mod rpc;
mod server;

use cluster::{
    implement::{NodeAddr, NodeId, ServerSdn, ServerSdnConfig},
    INNER_GATEWAY_SERVICE, MEDIA_SERVER_SERVICE,
};

#[cfg(feature = "gateway")]
use server::gateway::run_gateway_server;
#[cfg(feature = "rtmp")]
use server::rtmp::run_rtmp_server;
#[cfg(feature = "webrtc")]
use server::sip::run_sip_server;
#[cfg(feature = "webrtc")]
use server::webrtc::run_webrtc_server;

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Media Server node
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Http port
    #[arg(env, long, default_value_t = 3000)]
    http_port: u16,

    /// Current Node ID
    #[arg(env, long, default_value_t = 1)]
    node_id: NodeId,

    /// Current Node ID
    #[arg(env, long, default_value_t = 100)]
    max_conn: u64,

    /// Neighbors
    #[arg(env, long)]
    seeds: Vec<NodeAddr>,

    #[command(subcommand)]
    server: Servers,
}

#[derive(Debug, Subcommand)]
enum Servers {
    #[cfg(feature = "gateway")]
    Gateway(server::gateway::GatewayArgs),
    #[cfg(feature = "webrtc")]
    Webrtc(server::webrtc::WebrtcArgs),
    #[cfg(feature = "rtmp")]
    Rtmp(server::rtmp::RtmpArgs),
    #[cfg(feature = "sip")]
    Sip(server::sip::SipArgs),
}

#[async_std::main]
async fn main() {
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();

    match args.server {
        #[cfg(feature = "gateway")]
        Servers::Gateway(opts) => {
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, INNER_GATEWAY_SERVICE, ServerSdnConfig { seeds: args.seeds }).await;
            if let Err(e) = run_gateway_server(args.http_port, opts, cluster, rpc_endpoint).await {
                log::error!("[GatewayServer] error {}", e);
            }
        }
        #[cfg(feature = "webrtc")]
        Servers::Webrtc(opts) => {
            use server::MediaServerContext;
            let ctx = MediaServerContext::new(args.node_id, args.max_conn);
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, MEDIA_SERVER_SERVICE, ServerSdnConfig { seeds: args.seeds }).await;
            if let Err(e) = run_webrtc_server(args.http_port, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[WebrtcServer] error {}", e);
            }
        }
        #[cfg(feature = "rtmp")]
        Servers::Rtmp(opts) => {
            use server::MediaServerContext;
            let ctx = MediaServerContext::new(args.node_id, args.max_conn);
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, MEDIA_SERVER_SERVICE, ServerSdnConfig { seeds: args.seeds }).await;
            if let Err(e) = run_rtmp_server(args.http_port, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[RtmpServer] error {}", e);
            }
        }
        #[cfg(feature = "sip")]
        Servers::Sip(opts) => {
            use server::MediaServerContext;
            let ctx = MediaServerContext::new(args.node_id, args.max_conn);
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, MEDIA_SERVER_SERVICE, ServerSdnConfig { seeds: args.seeds }).await;
            if let Err(e) = run_sip_server(args.http_port, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[RtmpServer] error {}", e);
            }
        }
    }
}
