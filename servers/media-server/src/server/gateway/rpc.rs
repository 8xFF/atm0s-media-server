use ::cluster::rpc::gateway::{NodePing, NodePong, QueryBestNodesRequest, QueryBestNodesResponse};
use ::cluster::rpc::{general::*, webrtc::*, whep::*, whip::*, RpcReqRes};

pub mod cluster;
pub mod http;

pub enum RpcEvent {
    NodePing(Box<dyn RpcReqRes<NodePing, NodePong>>),
    BestNodes(Box<dyn RpcReqRes<QueryBestNodesRequest, QueryBestNodesResponse>>),
    WhipConnect(Box<dyn RpcReqRes<WhipConnectRequest, WhipConnectResponse>>),
    WhepConnect(Box<dyn RpcReqRes<WhepConnectRequest, WhepConnectResponse>>),
    WebrtcConnect(Box<dyn RpcReqRes<WebrtcConnectRequest, WebrtcConnectResponse>>),
    WebrtcRemoteIce(Box<dyn RpcReqRes<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>>),
    WebrtcSdpPatch(Box<dyn RpcReqRes<WebrtcPatchRequest, WebrtcPatchResponse>>),
    MediaEndpointClose(Box<dyn RpcReqRes<MediaEndpointCloseRequest, MediaEndpointCloseResponse>>),
}
