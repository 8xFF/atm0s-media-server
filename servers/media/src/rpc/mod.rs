use transport_webrtc::{WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WhipConnectResponse};

pub mod http;

pub enum RpcEvent {
    WhipConnect(String, String, transport::RpcResponse<WhipConnectResponse>),
    WebrtcConnect(WebrtcConnectRequest, transport::RpcResponse<WebrtcConnectResponse>),
    WebrtcRemoteIce(WebrtcRemoteIceRequest, transport::RpcResponse<()>),
}
