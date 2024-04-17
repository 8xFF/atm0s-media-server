use std::net::IpAddr;

use super::RpcResult;

#[derive(Debug, Clone)]
pub struct WhipConnectReq {
    pub sdp: String,
    pub token: String,
    pub ip: IpAddr,
    pub user_agent: String,
}

#[derive(Debug, Clone)]
pub struct WhipConnectRes {
    pub conn_id: String,
    pub sdp: String,
}

#[derive(Debug, Clone)]
pub struct WhipRemoteIceReq {
    pub conn_id: String,
    pub ice: String,
}

#[derive(Debug, Clone)]
pub struct WhipRemoteIceRes {}

#[derive(Debug, Clone)]
pub struct WhipDeleteReq {
    pub conn_id: String,
}

#[derive(Debug, Clone)]
pub struct WhipDeleteRes {}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcReq {
    Connect(WhipConnectReq),
    RemoteIce(WhipRemoteIceReq),
    Delete(WhipDeleteReq),
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcRes {
    Connect(RpcResult<WhipConnectRes>),
    RemoteIce(RpcResult<WhipRemoteIceRes>),
    Delete(RpcResult<WhipDeleteRes>),
}
