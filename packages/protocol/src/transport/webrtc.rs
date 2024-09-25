use std::net::IpAddr;

use super::{ConnLayer, RpcResult};
use crate::{
    endpoint::AppId,
    protobuf::gateway::{ConnectRequest, ConnectResponse, RemoteIceRequest, RemoteIceResponse},
};

#[derive(Debug, Clone)]
pub enum RpcReq<Conn> {
    /// Ip, Agent, Req, Userdata, Record
    Connect(u64, AppId, IpAddr, String, ConnectRequest, Option<String>, bool),
    RemoteIce(Conn, RemoteIceRequest),
    /// ConnId, Ip, Agent, Req, Userdata, Record
    RestartIce(Conn, AppId, IpAddr, String, ConnectRequest, Option<String>, bool),
    Delete(Conn),
}

impl<Conn: ConnLayer> RpcReq<Conn> {
    pub fn down(self) -> (RpcReq<Conn::Down>, Option<Conn::DownRes>) {
        match self {
            RpcReq::Connect(session_id, app, ip_addr, user_agent, req, extra_data, record) => (RpcReq::Connect(session_id, app, ip_addr, user_agent, req, extra_data, record), None),
            RpcReq::RemoteIce(conn, req) => {
                let (down, layer) = conn.down();
                (RpcReq::RemoteIce(down, req), Some(layer))
            }
            RpcReq::RestartIce(conn, app, ip_addr, user_agent, req, extra_data, record) => {
                let (down, layer) = conn.down();
                (RpcReq::RestartIce(down, app, ip_addr, user_agent, req, extra_data, record), Some(layer))
            }
            RpcReq::Delete(conn) => {
                let (down, layer) = conn.down();
                (RpcReq::Delete(down), Some(layer))
            }
        }
    }

    pub fn get_down_part(&self) -> Option<Conn::DownRes> {
        match self {
            RpcReq::Connect(..) => None,
            RpcReq::RemoteIce(conn, ..) => Some(conn.get_down_part()),
            RpcReq::RestartIce(conn, ..) => Some(conn.get_down_part()),
            RpcReq::Delete(conn, ..) => Some(conn.get_down_part()),
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
