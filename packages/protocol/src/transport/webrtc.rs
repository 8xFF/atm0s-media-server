use std::net::IpAddr;

use super::{ConnLayer, RpcResult};
use crate::protobuf::gateway::{ConnectRequest, ConnectResponse, RemoteIceRequest, RemoteIceResponse};

#[derive(Debug, Clone)]
pub enum RpcReq<Conn> {
    /// Ip, Token, Agent, Req
    Connect(IpAddr, String, ConnectRequest),
    RemoteIce(Conn, RemoteIceRequest),
    RestartIce(Conn, IpAddr, String, String, ConnectRequest),
    Delete(Conn),
}

impl<Conn: ConnLayer> RpcReq<Conn> {
    pub fn down(self) -> (RpcReq<Conn::Down>, Option<Conn::DownRes>) {
        match self {
            RpcReq::Connect(ip_addr, user_agent, req) => (RpcReq::Connect(ip_addr, user_agent, req), None),
            RpcReq::RemoteIce(conn, req) => {
                let (down, layer) = conn.down();
                (RpcReq::RemoteIce(down, req), Some(layer))
            }
            RpcReq::RestartIce(conn, ip_addr, token, user_agent, req) => {
                let (down, layer) = conn.down();
                (RpcReq::RestartIce(down, ip_addr, token, user_agent, req), Some(layer))
            }
            RpcReq::Delete(conn) => {
                let (down, layer) = conn.down();
                (RpcReq::Delete(down), Some(layer))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum RpcRes<Conn> {
    Connect(RpcResult<(Conn, ConnectResponse)>),
    RemoteIce(RpcResult<RemoteIceResponse>),
    RestartIce(RpcResult<(Conn, ConnectResponse)>),
    Delete(RpcResult<()>),
}

impl<Conn: ConnLayer> RpcRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RpcRes<Conn::Up> {
        match self {
            RpcRes::Connect(Ok((conn, res))) => RpcRes::Connect(Ok((conn.up(param), res))),
            RpcRes::Connect(Err(e)) => RpcRes::Connect(Err(e)),
            RpcRes::RemoteIce(res) => RpcRes::RemoteIce(res),
            RpcRes::RestartIce(Ok((conn, res))) => RpcRes::RestartIce(Ok((conn.up(param), res))),
            RpcRes::RestartIce(Err(e)) => RpcRes::RestartIce(Err(e)),
            RpcRes::Delete(res) => RpcRes::Delete(res),
        }
    }
}
