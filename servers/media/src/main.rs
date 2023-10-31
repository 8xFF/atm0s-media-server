use clap::Parser;

mod rpc;
mod server;

use cluster::{Cluster, ClusterEndpoint};
use cluster_bluesea::{NodeAddr, NodeId, ServerBluesea, ServerBlueseaConfig};
use cluster_local::ServerLocal;
use server::WebrtcServer;

/// Media Server node
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Http port
    #[arg(env, long, default_value_t = 3000)]
    http_port: u16,

    /// Current Node ID
    #[arg(env, long)]
    node_id: Option<NodeId>,

    /// Neighbors
    #[arg(env, long)]
    neighbours: Vec<NodeAddr>,
}

#[async_std::main]
async fn main() {
    let args: Args = Args::parse();
    env_logger::builder().format_module_path(false).format_timestamp_millis().init();

    async fn start_server<C, CR>(cluster: C)
    where
        C: Cluster<CR>,
        CR: ClusterEndpoint + 'static,
    {
        let mut http_server = rpc::http::HttpRpcServer::new(3000);
        http_server.start().await;
        let mut server = WebrtcServer::<C, CR>::new(cluster);
        while let Some(event) = http_server.recv().await {
            server.on_incomming(event).await;
        }
    }

    match args.node_id {
        Some(node_id) => {
            let cluster = ServerBluesea::new(node_id, ServerBlueseaConfig { neighbours: args.neighbours }).await;
            start_server(cluster).await;
        }
        None => {
            let cluster = ServerLocal::new();
            start_server(cluster).await;
        }
    }
}
