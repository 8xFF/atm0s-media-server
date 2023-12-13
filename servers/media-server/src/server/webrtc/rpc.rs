use ::cluster::rpc::{
    gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
    general::*,
    webrtc::*,
    whep::*,
    whip::*,
    RpcReqRes,
};

pub mod cluster;
pub mod http;

pub enum RpcEvent {
    NodeHeathcheck(Box<dyn RpcReqRes<NodeHealthcheckRequest, NodeHealthcheckResponse>>),
    WhipConnect(Box<dyn RpcReqRes<WhipConnectRequest, WhipConnectResponse>>),
    WhepConnect(Box<dyn RpcReqRes<WhepConnectRequest, WhepConnectResponse>>),
    WebrtcConnect(Box<dyn RpcReqRes<WebrtcConnectRequest, WebrtcConnectResponse>>),
    WebrtcRemoteIce(Box<dyn RpcReqRes<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>>),
    WebrtcSdpPatch(Box<dyn RpcReqRes<WebrtcPatchRequest, WebrtcPatchResponse>>),
    MediaEndpointClose(Box<dyn RpcReqRes<MediaEndpointCloseRequest, MediaEndpointCloseResponse>>),
}
