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
pub struct WhipConnectRes<Conn> {
    pub conn_id: Conn,
    pub sdp: String,
}

#[derive(Debug, Clone)]
pub struct WhipRemoteIceReq<Conn> {
    pub conn_id: Conn,
    pub ice: String,
}

#[derive(Debug, Clone)]
pub struct WhipRemoteIceRes {}

#[derive(Debug, Clone)]
pub struct WhipDeleteReq<Conn> {
    pub conn_id: Conn,
}

#[derive(Debug, Clone)]
pub struct WhipDeleteRes {}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcReq<Conn> {
    Connect(WhipConnectReq),
    RemoteIce(WhipRemoteIceReq<Conn>),
    Delete(WhipDeleteReq<Conn>),
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcRes<Conn> {
    Connect(RpcResult<WhipConnectRes<Conn>>),
    RemoteIce(RpcResult<WhipRemoteIceRes>),
    Delete(RpcResult<WhipDeleteRes>),
}
