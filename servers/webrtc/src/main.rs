use cluster_local::ServerLocal;
use server::WebrtcServer;

pub(crate) mod rpc;
mod server;
mod transport;

#[async_std::main]
async fn main() {
    env_logger::builder().format_timestamp_millis().init();
    let mut http_server = rpc::http::HttpRpcServer::new(3000);
    http_server.start().await;
    let cluster = ServerLocal::new();
    let mut server = WebrtcServer::new(cluster);
    while let Some(event) = http_server.recv().await {
        server.on_incomming(event);
    }
}
