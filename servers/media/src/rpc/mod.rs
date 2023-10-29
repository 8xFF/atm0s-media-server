use transport_webrtc::{WebrtcConnectRequest, WebrtcRemoteIceRequest, WhipConnectResponse, WebrtcConnectResponse};

pub mod http;

pub enum RpcEvent {
    WhipConnect(String, String, transport::RpcResponse<WhipConnectResponse>),
    WebrtcConnect(WebrtcConnectRequest, transport::RpcResponse<WebrtcConnectResponse>),
    WebrtcRemoteIce(WebrtcRemoteIceRequest, transport::RpcResponse<()>),
}