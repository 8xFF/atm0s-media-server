use std::net::SocketAddr;

use atm0s_sdn::NodeId;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceInfo {
    pub usage: u8,
    pub max: u32,
    pub addr: Option<SocketAddr>,
    pub domain: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct NodePing {
    pub node_id: u32,
    pub token: String,
    pub webrtc: Option<ServiceInfo>,
    pub rtmp: Option<ServiceInfo>,
    pub sip: Option<ServiceInfo>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct NodePong {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub enum NodeHealthcheckRequest {
    Webrtc { max_send_bitrate: u32, max_recv_bitrate: u32 },
    Rtmp { transcode: bool },
    Sip { transcode: bool },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct NodeHealthcheckResponse {
    pub success: bool,
}

pub fn create_conn_id(node_id: NodeId, uuid: u64) -> String {
    format!("{}:{}", node_id, uuid)
}

pub fn parse_conn_id(conn_id: &str) -> Option<(NodeId, u64)> {
    let parts = conn_id.split(':').into_iter().collect::<Vec<_>>();
    let node_id = parts.get(0)?.parse().ok()?;
    let uuid = parts.get(1)?.parse().ok()?;
    Some((node_id, uuid))
}

//TODO test this
