use std::fmt::Display;

use crate::endpoint::{ClusterConnId, ServerConnId};

pub mod webrtc;
pub mod whep;
pub mod whip;

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcReq<Conn> {
    // Webrtc(webrtc::RpcReq),
    // Whep(whep::RpcReq),
    Whip(whip::RpcReq<Conn>),
}

impl RpcReq<ClusterConnId> {
    pub fn extract(self) -> (RpcReq<ServerConnId>, Option<u32>) {
        todo!()
    }
}

impl RpcReq<ServerConnId> {
    pub fn extract(self) -> (RpcReq<usize>, Option<u16>) {
        todo!()
    }
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcRes<Conn> {
    // Webrtc(webrtc::RpcRes),
    // Whep(whep::RpcRes),
    Whip(whip::RpcRes<Conn>),
}

impl RpcRes<ServerConnId> {
    pub fn up_layer(self, node: u32) -> RpcRes<ClusterConnId> {
        todo!()
    }
}

impl RpcRes<usize> {
    pub fn up_layer(self, worker: u16) -> RpcRes<ServerConnId> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: u16,
    pub message: String,
}

impl Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Code: {}, Message: {}", self.code, self.message)
    }
}

impl RpcError {
    pub fn new<C: Into<u16>>(code: C, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
        }
    }

    pub fn new2<C: Into<u16> + ToString>(code: C) -> Self {
        Self {
            message: code.to_string(),
            code: code.into(),
        }
    }
}

pub type RpcResult<Type> = Result<Type, RpcError>;
