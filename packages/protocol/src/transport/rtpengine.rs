use crate::{
    endpoint::{PeerId, RoomId},
    multi_tenancy::AppContext,
    protobuf,
};

use super::{ConnLayer, RpcResult};

#[derive(Debug, Clone)]
pub struct RtpCreateOfferRequest {
    pub app: AppContext,
    pub session_id: u64,
    pub room: RoomId,
    pub peer: PeerId,
    pub record: bool,
    pub extra_data: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RtpSetAnswerRequest {
    pub sdp: String,
}

#[derive(Debug, Clone)]
pub struct RtpCreateAnswerRequest {
    pub app: AppContext,
    pub session_id: u64,
    pub room: RoomId,
    pub peer: PeerId,
    pub sdp: String,
    pub record: bool,
    pub extra_data: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RpcReq<Conn> {
    CreateOffer(RtpCreateOfferRequest),
    CreateAnswer(RtpCreateAnswerRequest),
    SetAnswer(Conn, RtpSetAnswerRequest),
    Delete(Conn),
}

impl<Conn: ConnLayer> RpcReq<Conn> {
    pub fn down(self) -> (RpcReq<Conn::Down>, Option<Conn::DownRes>) {
        match self {
            RpcReq::CreateOffer(conn_req) => (RpcReq::CreateOffer(conn_req.clone()), None),
            RpcReq::SetAnswer(conn, req) => {
                let (down, layer) = conn.down();
                (RpcReq::SetAnswer(down, req), Some(layer))
            }
            RpcReq::CreateAnswer(conn_req) => (RpcReq::CreateAnswer(conn_req.clone()), None),
            RpcReq::Delete(conn) => {
                let (down, layer) = conn.down();
                (RpcReq::Delete(down), Some(layer))
            }
        }
    }

    pub fn get_down_part(&self) -> Option<Conn::DownRes> {
        match self {
            RpcReq::CreateOffer(..) => None,
            RpcReq::SetAnswer(conn, ..) => Some(conn.get_down_part()),
            RpcReq::CreateAnswer(..) => None,
            RpcReq::Delete(conn, ..) => Some(conn.get_down_part()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RpcRes<Conn> {
    CreateOffer(RpcResult<(Conn, String)>),
    SetAnswer(RpcResult<Conn>),
    CreateAnswer(RpcResult<(Conn, String)>),
    Delete(RpcResult<Conn>),
}

impl<Conn: ConnLayer> RpcRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RpcRes<Conn::Up> {
        match self {
            RpcRes::CreateOffer(res) => RpcRes::CreateOffer(res.map(|(conn, sdp)| (conn.up(param), sdp))),
            RpcRes::SetAnswer(res) => RpcRes::SetAnswer(res.map(|conn| conn.up(param))),
            RpcRes::CreateAnswer(res) => RpcRes::CreateAnswer(res.map(|(conn, sdp)| (conn.up(param), sdp))),
            RpcRes::Delete(res) => RpcRes::Delete(res.map(|conn| conn.up(param))),
        }
    }
}

impl TryFrom<protobuf::cluster_gateway::RtpEngineCreateOfferRequest> for RtpCreateOfferRequest {
    type Error = ();
    fn try_from(value: protobuf::cluster_gateway::RtpEngineCreateOfferRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            app: value.app.into(),
            session_id: value.session_id,
            room: value.room.into(),
            peer: value.peer.into(),
            record: value.record,
            extra_data: value.extra_data,
        })
    }
}

impl From<RtpCreateOfferRequest> for protobuf::cluster_gateway::RtpEngineCreateOfferRequest {
    fn from(val: RtpCreateOfferRequest) -> Self {
        protobuf::cluster_gateway::RtpEngineCreateOfferRequest {
            app: Some(val.app.into()),
            session_id: val.session_id,
            room: val.room.into(),
            peer: val.peer.into(),
            record: val.record,
            extra_data: val.extra_data,
        }
    }
}

impl TryFrom<protobuf::cluster_gateway::RtpEngineCreateAnswerRequest> for RtpCreateAnswerRequest {
    type Error = ();
    fn try_from(value: protobuf::cluster_gateway::RtpEngineCreateAnswerRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            app: value.app.into(),
            session_id: value.session_id,
            sdp: value.sdp,
            room: value.room.into(),
            peer: value.peer.into(),
            record: value.record,
            extra_data: value.extra_data,
        })
    }
}

impl From<RtpCreateAnswerRequest> for protobuf::cluster_gateway::RtpEngineCreateAnswerRequest {
    fn from(val: RtpCreateAnswerRequest) -> Self {
        protobuf::cluster_gateway::RtpEngineCreateAnswerRequest {
            app: Some(val.app.into()),
            session_id: val.session_id,
            sdp: val.sdp,
            room: val.room.into(),
            peer: val.peer.into(),
            record: val.record,
            extra_data: val.extra_data,
        }
    }
}
