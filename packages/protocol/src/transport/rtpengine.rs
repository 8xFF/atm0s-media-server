use crate::endpoint::{PeerId, RoomId};

use super::{ConnLayer, RpcResult};

#[derive(Debug, Clone)]
pub struct RtpConnectRequest {
    pub call_id: RoomId,
    pub leg_id: PeerId,
    pub sdp: String,
    pub session_id: u64,
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
    Connect(RpcResult<(PeerId, Conn, String)>),
    Delete(RpcResult<PeerId>),
}

impl<Conn: ConnLayer> RpcRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RpcRes<Conn::Up> {
        match self {
            RpcRes::Connect(Ok((peer_id, conn, sdp))) => RpcRes::Connect(Ok((peer_id, conn.up(param), sdp))),
            RpcRes::Connect(Err(e)) => RpcRes::Connect(Err(e)),
            RpcRes::Delete(res) => RpcRes::Delete(res),
        }
    }
}
