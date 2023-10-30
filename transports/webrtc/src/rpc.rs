use std::fmt::Debug;

use media_utils::{EndpointSubscribeScope, MixMinusAudioMode, PayloadType, RemoteBitrateControlMode};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

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
