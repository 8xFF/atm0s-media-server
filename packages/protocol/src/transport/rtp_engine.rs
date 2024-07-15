use super::{ConnLayer, RpcResult};

pub type CallId = String;
pub type LegId = String;

#[derive(Debug, Clone)]
pub struct RtpConnectRequest {
    pub call_id: CallId,
    pub leg_id: LegId,
    pub sdp: String,
}

#[derive(Debug, Clone)]
pub enum RtpReq<Conn> {
    Ping,
    Connect(RtpConnectRequest),
    End(Conn, u64),
}

impl<Conn: ConnLayer> RtpReq<Conn> {
    pub fn down(self) -> (RtpReq<Conn::Down>, Option<Conn::DownRes>) {
        match self {
            RtpReq::Ping => (RtpReq::Ping, None),
            RtpReq::Connect(conn_req) => (RtpReq::Connect(conn_req.clone()), None),
            RtpReq::End(conn, call_id) => {
                let (down, layer) = conn.down();
                (RtpReq::End(down, call_id), Some(layer))
            }
        }
    }

    pub fn get_down_part(&self) -> Option<Conn::DownRes> {
        match self {
            RtpReq::Ping => None,
            RtpReq::Connect(..) => None,
            RtpReq::End(conn, ..) => Some(conn.get_down_part()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RtpRes<Conn> {
    Ping(RpcResult<String>),
    Connect(RpcResult<(Conn, String)>),
    End(RpcResult<()>),
}

impl<Conn: ConnLayer> RtpRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RtpRes<Conn::Up> {
        match self {
            RtpRes::Ping(Ok(res)) => RtpRes::Ping(Ok(res)),
            RtpRes::Ping(Err(e)) => RtpRes::Ping(Err(e)),
            RtpRes::Connect(Ok((conn, sdp))) => RtpRes::Connect(Ok((conn.up(param), sdp))),
            RtpRes::Connect(Err(e)) => RtpRes::Connect(Err(e)),
            RtpRes::End(res) => RtpRes::End(res),
        }
    }
}
