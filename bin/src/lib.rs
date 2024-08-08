use std::net::SocketAddr;

use atm0s_sdn::{NodeAddr, NodeId};
use media_server_protocol::cluster::ZoneId;

mod errors;
mod http;
mod ng_controller;
#[cfg(feature = "node_metrics")]
mod node_metrics;
#[cfg(feature = "quinn_vnet")]
mod quinn;
mod rpc;
pub mod server;

#[derive(Clone)]
pub struct NodeConfig {
    pub node_id: NodeId,
    pub secret: String,
    pub seeds: Vec<NodeAddr>,
    pub bind_addrs: Vec<SocketAddr>,
    pub zone: ZoneId,
    pub bind_addrs_alt: Vec<SocketAddr>,
}
