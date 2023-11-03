use async_std::net::TcpStream;
use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn,
};
use transport_rtmp::RtmpTransport;
use transport_webrtc::{WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WhipConnectResponse};

pub mod http;

pub enum RpcEvent {
    WhipConnect(String, String, transport::RpcResponse<WhipConnectResponse>),
    WebrtcConnect(WebrtcConnectRequest, transport::RpcResponse<WebrtcConnectResponse>),
    WebrtcRemoteIce(WebrtcRemoteIceRequest, transport::RpcResponse<()>),
    RtmpConnect(RtmpTransport<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, String, String),
}
