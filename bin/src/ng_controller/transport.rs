mod udp;

use std::net::SocketAddr;

use super::commands::{NgRequest, NgResponse};

pub enum NgTransportType {
    Udp,
}

#[async_trait::async_trait]
pub trait NgTransport: Sync + Send {
    async fn send(&self, res: NgResponse, addr: SocketAddr);
    async fn recv(&self) -> Option<(NgRequest, SocketAddr)>;
}

pub async fn new_transport(transport: NgTransportType, port: u16) -> Box<dyn NgTransport> {
    match transport {
        NgTransportType::Udp => {
            let transport = udp::NgUdpTransport::new(port).await;
            Box::new(transport)
        }
    }
}
