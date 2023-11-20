use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn,
};
use transport_rtmp::RtmpTransport;
use transport_webrtc::{WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WhepConnectResponse, WhipConnectResponse};

pub mod http;

pub enum RpcEvent {
    WhipConnect(String, String, transport::RpcResponse<WhipConnectResponse>),
    WhipPatch(String, String, transport::RpcResponse<String>),
    WhipClose(String, transport::RpcResponse<()>),
    WhepConnect(String, String, transport::RpcResponse<WhepConnectResponse>),
    WhepPatch(String, String, transport::RpcResponse<String>),
    WhepClose(String, transport::RpcResponse<()>),
    WebrtcConnect(WebrtcConnectRequest, transport::RpcResponse<WebrtcConnectResponse>),
    WebrtcRemoteIce(WebrtcRemoteIceRequest, transport::RpcResponse<()>),
    RtmpConnect(RtmpTransport<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, String, String),
}
