use std::{fmt::Display, hash::Hash};

use derive_more::{Display, From};
use serde::{Deserialize, Serialize};

use crate::protobuf;

pub mod rtpengine;
pub mod webrtc;
pub mod whep;
pub mod whip;

/// RemoteTrackId is used for track which received media from client
#[derive(From, Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize)]
pub struct RemoteTrackId(pub u16);

impl Hash for RemoteTrackId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// LocalTrackId is used for track which send media to client
#[derive(From, Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize)]
pub struct LocalTrackId(pub u16);

impl Hash for LocalTrackId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

pub trait ConnLayer {
    type Up;
    type UpParam;
    type Down;
    type DownRes;

    fn down(self) -> (Self::Down, Self::DownRes);
    fn up(self, param: Self::UpParam) -> Self::Up;
    fn get_down_part(&self) -> Self::DownRes;
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcReq<Conn> {
    Whep(whep::RpcReq<Conn>),
    Whip(whip::RpcReq<Conn>),
    Webrtc(webrtc::RpcReq<Conn>),
    RtpEngine(rtpengine::RpcReq<Conn>),
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
            Self::RtpEngine(req) => {
                let (req, layer) = req.down();
                (RpcReq::RtpEngine(req), layer)
            }
        }
    }

    pub fn get_conn_part(&self) -> Option<Conn::DownRes> {
        match self {
            Self::Whip(req) => req.get_down_part(),
            Self::Whep(req) => req.get_down_part(),
            Self::Webrtc(req) => req.get_down_part(),
            Self::RtpEngine(req) => req.get_down_part(),
        }
    }
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcRes<Conn> {
    Whep(whep::RpcRes<Conn>),
    Whip(whip::RpcRes<Conn>),
    Webrtc(webrtc::RpcRes<Conn>),
    RtpEngine(rtpengine::RpcRes<Conn>),
}

impl<Conn: ConnLayer> RpcRes<Conn> {
    pub fn up(self, param: Conn::UpParam) -> RpcRes<Conn::Up> {
        match self {
            Self::Whip(req) => RpcRes::Whip(req.up(param)),
            Self::Whep(req) => RpcRes::Whep(req.up(param)),
            Self::Webrtc(req) => RpcRes::Webrtc(req.up(param)),
            Self::RtpEngine(req) => RpcRes::RtpEngine(req.up(param)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl From<RpcError> for protobuf::shared::Error {
    fn from(val: RpcError) -> Self {
        protobuf::shared::Error { code: val.code, message: val.message }
    }
}

pub type RpcResult<Type> = Result<Type, RpcError>;
