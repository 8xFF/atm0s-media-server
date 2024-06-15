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
}
