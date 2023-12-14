use ::cluster::rpc::gateway::{NodePing, NodePong};
use ::cluster::rpc::{general::*, webrtc::*, whep::*, whip::*, RpcReqRes};

pub mod cluster;
pub mod http;

pub enum RpcEvent {
    NodePing(Box<dyn RpcReqRes<NodePing, NodePong>>),
    WhipConnect(Box<dyn RpcReqRes<WhipConnectRequest, WhipConnectResponse> + Sync>),
    WhepConnect(Box<dyn RpcReqRes<WhepConnectRequest, WhepConnectResponse> + Sync>),
    WebrtcConnect(Box<dyn RpcReqRes<WebrtcConnectRequest, WebrtcConnectResponse> + Sync>),
    WebrtcRemoteIce(Box<dyn RpcReqRes<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>>),
    WebrtcSdpPatch(Box<dyn RpcReqRes<WebrtcPatchRequest, WebrtcPatchResponse>>),
    MediaEndpointClose(Box<dyn RpcReqRes<MediaEndpointCloseRequest, MediaEndpointCloseResponse>>),
}
