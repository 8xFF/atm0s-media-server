use std::net::{IpAddr, SocketAddr};

use atm0s_sdn::NodeId;
use media_utils::F32;
use poem_openapi::Object;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

use super::general::MediaSessionProtocol;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceInfo {
    pub usage: u8,
    pub live: u32,
    pub max: u32,
    pub addr: Option<SocketAddr>,
    pub domain: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct NodePing {
    pub node_id: u32,
    pub zone: String,
    pub location: Option<(F32<2>, F32<2>)>,
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

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct QueryBestNodesRequest {
    pub ip_addr: IpAddr,
    pub protocol: MediaSessionProtocol,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct QueryBestNodesResponse {
    pub nodes: Vec<NodeId>,
    pub service_id: u8,
}

//TODO test this
