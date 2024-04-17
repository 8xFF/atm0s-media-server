use atm0s_sdn::{NodeAddr, NodeId, SdnWorkerCfg};
use clap::Parser;
use server::{run_media_connector, run_media_gateway, run_media_server, MediaSdnConfig, ServerType};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod http;
mod server;

/// Scalable Media Server solution for WebRTC, RTMP, and SIP.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Http port
    #[arg(env, long)]
    http_port: Option<u16>,

    /// Run http with tls or not
    #[arg(env, long)]
    http_tls: Option<u16>,

    /// Sdn port
    #[arg(env, long, default_value_t = 0)]
    sdn_port: u16,

    /// Sdn Zone
    #[arg(env, long, default_value = "local")]
    sdn_zone: String,

    /// Current Node ID
    #[arg(env, long, default_value_t = 1)]
    node_id: NodeId,

    /// Cluster Secret Key
    #[arg(env, long, default_value = "insecure")]
    secret: String,

    /// Neighbors
    #[arg(env, long)]
    seeds: Vec<NodeAddr>,

    #[command(subcommand)]
    server: ServerType,
}

#[tokio::main]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "atm0s_media_server=info");
    }
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();

    let sdn = MediaSdnConfig {
        node_id: args.node_id,
        seeds: args.seeds,
        secret: args.secret,
    };

    match args.server {
        ServerType::Gateway(args) => run_media_gateway(args).await,
        ServerType::Connector(args) => run_media_connector(args).await,
        ServerType::Media(args) => run_media_server(sdn, args).await,
    }
}
