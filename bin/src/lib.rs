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
    pub bind_addrs: Vec<SocketAddr>,
    pub zone: u32,
    pub bind_addrs_alt: Vec<SocketAddr>,
}
