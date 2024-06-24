use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNodeGenericInfo {
    pub addr: String,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterGatewayInfo {
    pub live: u32,
    pub max: u32,
    pub lat: f32,
    pub lon: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMediaInfo {
    pub live: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterNodeInfo {
    Console(ClusterNodeGenericInfo),
    Gateway(ClusterNodeGenericInfo, ClusterGatewayInfo),
    Media(ClusterNodeGenericInfo, ClusterMediaInfo),
    Connector(ClusterNodeGenericInfo),
}

/// Generate global cluster session_id
pub fn gen_cluster_session_id() -> u64 {
    rand::random::<u64>() & 0x7FFF_FFFF_FFFF_FFFF //avoid over i64, which some database will error
}
