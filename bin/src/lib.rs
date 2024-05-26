use atm0s_sdn::{NodeAddr, NodeId};

mod http;
mod quinn;
pub mod server;

#[derive(Clone)]
pub struct NodeConfig {
    pub node_id: NodeId,
    pub session: u64,
    pub secret: String,
    pub seeds: Vec<NodeAddr>,
    pub udp_port: u16,
}
