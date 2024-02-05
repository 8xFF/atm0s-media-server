use std::{net::IpAddr, ops::Deref};

use poem_openapi::{Enum, Object};
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct MediaEndpointCloseRequest {
    pub conn_id: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct MediaEndpointCloseResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize, Enum, PartialEq, Eq, Clone, Copy)]
pub enum MediaSessionProtocol {
    Whip,
    Whep,
    Webrtc,
    Rtmp,
    Sip,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct NodeInfo {
    pub node_id: u32,
    pub address: String,
    pub server_type: ServerType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ServerType {
    GATEWAY,
    CONNECTOR,
    SIP,
    WEBRTC,
    RTMP,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct RemoteAddr(IpAddr);

impl From<IpAddr> for RemoteAddr {
    fn from(addr: IpAddr) -> Self {
        Self(addr)
    }
}

impl Into<IpAddr> for RemoteAddr {
    fn into(self) -> IpAddr {
        self.0
    }
}

impl Deref for RemoteAddr {
    type Target = IpAddr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for RemoteAddr {
    fn default() -> Self {
        Self(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)))
    }
}
