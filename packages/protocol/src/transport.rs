pub mod webrtc;
pub mod whep;
pub mod whip;

pub enum RpcReq {
    Webrtc(webrtc::RpcReq),
    Whep(whep::RpcReq),
    Whip(whip::RpcReq),
}

pub enum RpcRes {
    Webrtc(webrtc::RpcRes),
    Whep(whep::RpcRes),
    Whip(whip::RpcRes),
}
