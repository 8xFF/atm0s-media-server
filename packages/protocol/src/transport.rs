use std::fmt::Display;

use crate::protobuf;

pub mod webrtc;
pub mod whep;
pub mod whip;

pub trait ConnLayer {
    type Up;
    type UpParam;
    type Down;
    type DownRes;

    fn down(self) -> (Self::Down, Self::DownRes);
    fn up(self, param: Self::UpParam) -> Self::Up;
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcReq<Conn> {
    Whep(whep::RpcReq<Conn>),
    Whip(whip::RpcReq<Conn>),
    Webrtc(webrtc::RpcReq<Conn>),
}

impl<Conn: ConnLayer> RpcReq<Conn> {
    pub fn down(self) -> (RpcReq<Conn::Down>, Option<Conn::DownRes>) {
        match self {
            Self::Whip(req) => {
                let (req, layer) = req.down();
                (RpcReq::Whip(req), layer)
            }
            Self::Whep(req) => {
                let (req, layer) = req.down();
                (RpcReq::Whep(req), layer)
            }
            Self::Webrtc(req) => {
                let (req, layer) = req.down();
                (RpcReq::Webrtc(req), layer)
            }
        }
    }
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcRes<Conn> {
    Whep(whep::RpcRes<Conn>),
    Whip(whip::RpcRes<Conn>),
    Webrtc(webrtc::RpcRes<Conn>),
}

impl<Conn: ConnLayer> RpcRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RpcRes<Conn::Up> {
        match self {
            Self::Whip(req) => RpcRes::Whip(req.up(param)),
            Self::Whep(req) => RpcRes::Whep(req.up(param)),
            Self::Webrtc(req) => RpcRes::Webrtc(req.up(param)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: u32,
    pub message: String,
}

impl Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Code: {}, Message: {}", self.code, self.message)
    }
}

impl RpcError {
    pub fn new<C: Into<u32>>(code: C, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
        }
    }

    pub fn new2<C: Into<u32> + Display>(code: C) -> Self {
        Self {
            message: code.to_string(),
            code: code.into(),
        }
    }
}

impl Into<protobuf::shared::Error> for RpcError {
    fn into(self) -> protobuf::shared::Error {
        protobuf::shared::Error {
            code: self.code,
            message: self.message,
        }
    }
}

pub type RpcResult<Type> = Result<Type, RpcError>;
