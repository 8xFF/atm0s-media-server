use crate::endpoint::{PeerId, RoomId};

use super::{ConnLayer, RpcResult};

#[derive(Debug, Clone)]
pub struct RtpConnectRequest {
    pub session_id: u64,
    pub room: RoomId,
    pub peer: PeerId,
    pub sdp: String,
    pub record: bool,
    pub extra_data: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RpcReq<Conn> {
    Connect(RtpConnectRequest),
    Delete(Conn),
}

impl<Conn: ConnLayer> RpcReq<Conn> {
    pub fn down(self) -> (RpcReq<Conn::Down>, Option<Conn::DownRes>) {
        match self {
            RpcReq::Connect(conn_req) => (RpcReq::Connect(conn_req.clone()), None),
            RpcReq::Delete(conn) => {
                let (down, layer) = conn.down();
                (RpcReq::Delete(down), Some(layer))
            }
        }
    }

    pub fn get_down_part(&self) -> Option<Conn::DownRes> {
        match self {
            RpcReq::Connect(..) => None,
            RpcReq::Delete(conn, ..) => Some(conn.get_down_part()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RpcRes<Conn> {
    Connect(RpcResult<(Conn, String)>),
    Delete(RpcResult<Conn>),
}

impl<Conn: ConnLayer> RpcRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RpcRes<Conn::Up> {
        match self {
            RpcRes::Connect(res) => RpcRes::Connect(res.map(|(conn, sdp)| (conn.up(param), sdp))),
            RpcRes::Delete(res) => RpcRes::Delete(res.map(|conn| conn.up(param))),
        }
    }
}
