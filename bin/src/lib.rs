use std::{net::SocketAddr, str::FromStr};

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

pub async fn fetch_node_addr_from_api(url: &str) -> Result<NodeAddr, String> {
    let resp = reqwest::get(format!("{}/api/node/address", url)).await.map_err(|e| e.to_string())?;
    let node_addr = resp
        .json::<http::Response<String>>()
        .await
        .map_err(|e| e.to_string())?
        .data
        .ok_or(format!("No data in response from {}", url))?;
    NodeAddr::from_str(&node_addr).map_err(|e| e.to_string())
}
