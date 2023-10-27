use std::fmt::Debug;

use async_std::channel::{bounded, Receiver, Sender};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use utils::{EndpointSubscribeScope, MixMinusAudioMode, PayloadType, RemoteBitrateControlMode, ServerError};

pub(crate) mod http;

#[derive(Clone)]
pub struct RpcResponse<T> {
    tx: Sender<(u16, Result<T, ServerError>)>,
}

impl<T> Debug for RpcResponse<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcResponse").finish()
    }
}

impl<T> Eq for RpcResponse<T> {}

impl<T> PartialEq for RpcResponse<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
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
    pub sub_scope: Option<EndpointSubscribeScope>,
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

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq)]
pub struct WebrtcRemoteIceRequest {
    pub node_id: u32,
    pub conn_id: String,
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
    pub username_fragment: Option<String>,
}

pub type WebrtcRemoteIceResponse = String;

pub struct WhipConnectResponse {
    pub location: String,
    pub sdp: String,
}

pub enum RpcEvent {
    WhipConnect(String, String, RpcResponse<WhipConnectResponse>),
    WebrtcConnect(WebrtcConnectRequest, RpcResponse<WebrtcConnectResponse>),
    WebrtcRemoteIce(WebrtcRemoteIceRequest, RpcResponse<()>),
}
