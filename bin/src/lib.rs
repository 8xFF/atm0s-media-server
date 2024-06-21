use std::net::SocketAddr;

use atm0s_sdn::{NodeAddr, NodeId};

mod errors;
mod http;
#[cfg(feature = "node_metrics")]
mod node_metrics;
#[cfg(feature = "quinn_vnet")]
mod quinn;
pub mod server;

#[derive(Clone)]
pub struct NodeConfig {
    pub node_id: NodeId,
    pub secret: String,
    pub seeds: Vec<NodeAddr>,
    pub udp_port: u16,
    pub zone: u32,
    pub custom_addrs: Vec<SocketAddr>,
}

use std::time::{SystemTime, UNIX_EPOCH};
pub fn now_ms() -> u64 {
    let start = SystemTime::now();
    start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis() as u64
}
