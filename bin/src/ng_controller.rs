mod commands;
mod server;
mod transport;

use media_server_protocol::{
    endpoint::ClusterConnId,
    transport::{RpcReq, RpcRes},
};
pub use server::{NgControllerServer, NgControllerServerConfig};
use tokio::sync::mpsc::Sender;
pub use transport::NgTransportType;

use crate::http::Rpc;

pub async fn run_ng_controller_server(port: u16, sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut ng_controller_server = NgControllerServer::new(
        NgControllerServerConfig {
            port,
            transport: NgTransportType::Udp,
        },
        sender,
    )
    .await;

    ng_controller_server.process().await;
    Ok(())
}
