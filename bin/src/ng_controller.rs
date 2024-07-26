mod commands;
mod server;
mod transport;

use media_server_protocol::{
    endpoint::ClusterConnId,
    transport::{RpcReq, RpcRes},
};
pub use server::NgControllerServer;
use tokio::sync::mpsc::Sender;
use transport::NgUdpTransport;

use crate::http::Rpc;

pub async fn run_ng_controller_server(port: u16, sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Result<(), Box<dyn std::error::Error>> {
    let udp_transport = NgUdpTransport::new(port).await;
    let mut ng_controller_server = NgControllerServer::new(udp_transport, sender).await;

    ng_controller_server.process().await;
    Ok(())
}
