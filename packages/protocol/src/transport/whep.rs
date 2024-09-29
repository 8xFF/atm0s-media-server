use std::net::IpAddr;

use crate::{
    endpoint::{PeerId, RoomId},
    multi_tenancy::AppContext,
    protobuf,
};

use super::{ConnLayer, RpcResult};

#[derive(Debug, Clone)]
pub struct WhepConnectReq {
    pub app: AppContext,
    pub session_id: u64,
    pub sdp: String,
    pub room: RoomId,
    pub peer: PeerId,
    pub ip: IpAddr,
    pub user_agent: String,
    pub extra_data: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WhepConnectRes<Conn> {
    pub conn_id: Conn,
    pub sdp: String,
}

#[derive(Debug, Clone)]
pub struct WhepRemoteIceReq<Conn> {
    pub conn_id: Conn,
    pub ice: String,
}

#[derive(Debug, Clone)]
pub struct WhepRemoteIceRes {}

#[derive(Debug, Clone)]
pub struct WhepDeleteReq<Conn> {
    pub conn_id: Conn,
}

#[derive(Debug, Clone)]
pub struct WhepDeleteRes {}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcReq<Conn> {
    Connect(WhepConnectReq),
    RemoteIce(WhepRemoteIceReq<Conn>),
    Delete(WhepDeleteReq<Conn>),
}

impl<Conn: ConnLayer> RpcReq<Conn> {
    pub fn down(self) -> (RpcReq<Conn::Down>, Option<Conn::DownRes>) {
        match self {
            RpcReq::Connect(req) => (RpcReq::Connect(req), None),
            RpcReq::RemoteIce(req) => {
                let (down, layer) = req.conn_id.down();
                (RpcReq::RemoteIce(WhepRemoteIceReq { conn_id: down, ice: req.ice }), Some(layer))
            }
            RpcReq::Delete(req) => {
                let (down, layer) = req.conn_id.down();
                (RpcReq::Delete(WhepDeleteReq { conn_id: down }), Some(layer))
            }
        }
    }

    pub fn get_down_part(&self) -> Option<Conn::DownRes> {
        match self {
            RpcReq::Connect(_req) => None,
            RpcReq::RemoteIce(req) => Some(req.conn_id.get_down_part()),
            RpcReq::Delete(req) => Some(req.conn_id.get_down_part()),
        }
    }
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcRes<Conn> {
    Connect(RpcResult<WhepConnectRes<Conn>>),
    RemoteIce(RpcResult<WhepRemoteIceRes>),
    Delete(RpcResult<WhepDeleteRes>),
}

impl<Conn: ConnLayer> RpcRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RpcRes<Conn::Up> {
        match self {
            RpcRes::Connect(Ok(res)) => RpcRes::Connect(Ok(WhepConnectRes {
                conn_id: res.conn_id.up(param),
                sdp: res.sdp,
            })),
            RpcRes::Connect(Err(e)) => RpcRes::Connect(Err(e)),
            RpcRes::RemoteIce(res) => RpcRes::RemoteIce(res),
            RpcRes::Delete(res) => RpcRes::Delete(res),
        }
    }
}

impl TryFrom<protobuf::cluster_gateway::WhepConnectRequest> for WhepConnectReq {
    type Error = ();
    fn try_from(value: protobuf::cluster_gateway::WhepConnectRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            app: value.app.into(),
            session_id: value.session_id,
            sdp: value.sdp,
            room: value.room.into(),
            peer: value.peer.into(),
            ip: value.ip.parse().map_err(|_| ())?,
            user_agent: value.user_agent,
            extra_data: value.extra_data,
        })
    }
}

impl From<WhepConnectReq> for protobuf::cluster_gateway::WhepConnectRequest {
    fn from(val: WhepConnectReq) -> Self {
        protobuf::cluster_gateway::WhepConnectRequest {
            app: Some(val.app.into()),
            session_id: val.session_id,
            user_agent: val.user_agent,
            ip: val.ip.to_string(),
            sdp: val.sdp,
            room: val.room.into(),
            peer: val.peer.into(),
            extra_data: val.extra_data,
        }
    }
}
