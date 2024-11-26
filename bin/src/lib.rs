use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use atm0s_sdn::{NodeAddr, NodeId};
use clap::ValueEnum;
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

/// Fetch node addrs from the given url.
/// The url should return a list of node addrs in JSON format or a single node addr.
pub async fn fetch_node_addrs_from_api(url: &str) -> Result<Vec<NodeAddr>, String> {
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    let content = resp.text().await.map_err(|e| e.to_string())?;
    if content.starts_with('[') {
        let node_addrs: Vec<String> = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(node_addrs.into_iter().flat_map(|addr| NodeAddr::from_str(&addr)).collect())
    } else {
        Ok(vec![NodeAddr::from_str(&content).map_err(|e| e.to_string())?])
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum CloudProvider {
    Aws,
    Gcp,
    Azure,
    Other,
}

pub async fn fetch_node_ip_alt_from_cloud(cloud: CloudProvider) -> Result<IpAddr, String> {
    match cloud {
        CloudProvider::Aws => {
            let resp = reqwest::get("http://169.254.169.254/latest/meta-data/public-ipv4").await.map_err(|e| e.to_string())?;
            let ip = resp.text().await.map_err(|e| e.to_string())?;
            IpAddr::from_str(ip.trim()).map_err(|e| e.to_string())
        }
        CloudProvider::Gcp => {
            let client = reqwest::Client::new();
            let resp = client
                .get("http://metadata/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip")
                .header("Metadata-Flavor", "Google")
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let ip = resp.text().await.map_err(|e| e.to_string())?;
            IpAddr::from_str(ip.trim()).map_err(|e| e.to_string())
        }
        CloudProvider::Azure | CloudProvider::Other => {
            let resp = reqwest::get("http://ipv4.icanhazip.com").await.map_err(|e| e.to_string())?;
            let ip = resp.text().await.map_err(|e| e.to_string())?;
            IpAddr::from_str(ip.trim()).map_err(|e| e.to_string())
        }
    }
}
