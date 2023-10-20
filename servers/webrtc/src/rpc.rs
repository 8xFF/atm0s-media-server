use async_std::channel::{bounded, Receiver, Sender};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use utils::{MixMinusAudioMode, PayloadType, RemoteBitrateControlMode, ServerError};

pub(crate) mod http;

pub struct RpcResponse<T> {
    tx: Sender<(u16, Result<T, ServerError>)>,
}

impl<T> RpcResponse<T> {
    pub fn new() -> (Self, Receiver<(u16, Result<T, ServerError>)>) {
        let (tx, rx) = bounded(1);
        (Self { tx }, rx)
    }
    pub fn answer(&mut self, code: u16, res: Result<T, ServerError>) {
        self.tx.send_blocking((code, res));
    }

    pub async fn answer_async(&mut self, code: u16, res: Result<T, ServerError>) {
        self.tx.send((code, res)).await;
    }
}

#[derive(Serialize, Deserialize, Debug, Object, PartialEq, Eq)]
pub struct WebrtcConnectRequestReceivers {
    pub audio: u8,
    pub video: u8,
}

#[derive(Serialize, Deserialize, Debug, Object, PartialEq, Eq)]
pub struct WebrtcConnectRequestSender {
    pub kind: String,
    pub name: String,
    pub uuid: String,
    pub label: String,
    pub screen: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq)]
pub struct WebrtcConnectRequest {
    pub version: Option<String>,
    pub room: String,
    pub peer: String,
    pub token: String,
    pub mix_minus_audio: Option<MixMinusAudioMode>,
    pub join_now: Option<bool>,
    pub codecs: Option<Vec<PayloadType>>,
    pub receivers: WebrtcConnectRequestReceivers,
    pub sdp: String,
    pub compressed_sdp: Option<Vec<u8>>,
    pub senders: Vec<WebrtcConnectRequestSender>,
    pub remote_bitrate_control_mode: Option<RemoteBitrateControlMode>,
}

pub struct WebrtcConnectResponse {
    pub sdp: String,
    pub conn_id: String,
}

pub struct WhipConnectResponse {
    pub location: String,
    pub sdp: String,
}

pub enum RpcEvent {
    WhipConnect(String, String, RpcResponse<WhipConnectResponse>),
    WebrtcConnect(WebrtcConnectRequest, RpcResponse<WebrtcConnectResponse>),
    WebrtcRemoteIce(String, String, RpcResponse<()>),
}
