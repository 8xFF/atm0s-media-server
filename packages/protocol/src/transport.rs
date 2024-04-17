pub mod webrtc;
pub mod whep;
pub mod whip;

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcReq {
    // Webrtc(webrtc::RpcReq),
    // Whep(whep::RpcReq),
    Whip(whip::RpcReq),
}

#[derive(Debug, Clone, convert_enum::From, convert_enum::TryInto)]
pub enum RpcRes {
    // Webrtc(webrtc::RpcRes),
    // Whep(whep::RpcRes),
    Whip(whip::RpcRes),
}

#[derive(Debug, Clone)]
pub enum RpcResult<Type> {
    Success(Type),
    Error { code: u16, message: String },
}
