use clap::Parser;
use cluster_bluesea::{NodeAddr, NodeId, ServerBluesea, ServerBlueseaConfig};
use cluster_local::ServerLocal;
use std::net::SocketAddr;

mod server;
mod sip_session;

/// Media Server node
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Current Node ID
    #[arg(env, long)]
    node_id: Option<NodeId>,

    /// Neighbors
    #[arg(env, long)]
    neighbours: Vec<NodeAddr>,

    /// Sip listen socket
    #[arg(env, long, default_value = "127.0.0.1:5060")]
    sip_addr: SocketAddr,
}

#[async_std::main]
async fn main() {
    let args: Args = Args::parse();
    env_logger::builder().format_module_path(false).format_timestamp_millis().init();
    match args.node_id {
        Some(node_id) => {
            let cluster = ServerBluesea::new(node_id, ServerBlueseaConfig { neighbours: args.neighbours }).await;
            server::start_server(cluster, args.sip_addr).await;
        }
        None => {
            let cluster = ServerLocal::new();
            server::start_server(cluster, args.sip_addr).await;
        }
    }
}
