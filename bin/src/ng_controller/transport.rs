mod udp;

use std::net::SocketAddr;

use super::commands::{NgRequest, NgResponse};
pub use udp::NgUdpTransport;

pub trait NgTransport {
    async fn send(&self, res: NgResponse, addr: SocketAddr);
    async fn recv(&self) -> Option<(NgRequest, SocketAddr)>;
}
