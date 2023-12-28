use std::fmt::Debug;

use crate::{MediaSessionToken, VerifyObject};

use super::super::media::{EndpointSubscribeScope, MixMinusAudioMode, PayloadType, RemoteBitrateControlMode};
use poem_openapi::Object;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Object, PartialEq, Eq, Clone)]
pub struct WebrtcConnectRequestReceivers {
    pub audio: u8,
    pub video: u8,
}

#[derive(Serialize, Deserialize, Debug, Object, PartialEq, Eq, Clone)]
pub struct WebrtcConnectRequestSender {
    //TODO switch to enum
    pub kind: String,
    pub name: String,
    pub uuid: String,
    pub label: String,
    pub screen: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WebrtcConnectRequest {
    pub session_uuid: Option<u64>,
    pub ip_addr: Option<String>,
    pub user_agent: Option<String>,
    pub version: Option<String>,
    pub room: String,
    pub peer: String,
    pub sub_scope: Option<EndpointSubscribeScope>,
    pub token: String,
    pub mix_minus_audio: Option<MixMinusAudioMode>,
    pub join_now: Option<bool>,
    pub codecs: Option<Vec<PayloadType>>,
    pub receivers: WebrtcConnectRequestReceivers,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
    pub senders: Vec<WebrtcConnectRequestSender>,
    pub remote_bitrate_control_mode: Option<RemoteBitrateControlMode>,
}

impl VerifyObject for WebrtcConnectRequest {
    fn verify(&self, verifier: &dyn crate::SessionTokenVerifier) -> Option<MediaSessionToken> {
        let token = verifier.verify_media_session(&self.token)?;
        if token.protocol != crate::rpc::general::MediaSessionProtocol::Webrtc {
            return None;
        }
        if token.room != self.room {
            return None;
        }
        if let Some(peer) = &token.peer {
            if !peer.eq(&self.peer) {
                return None;
            }
        }
        Some(token)
    }
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct WebrtcConnectResponse {
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
    pub conn_id: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WebrtcRemoteIceRequest {
    pub conn_id: String,
    pub candidate: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct WebrtcRemoteIceResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WebrtcPatchRequest {
    pub conn_id: String,
    pub sdp: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct WebrtcPatchResponse {
    pub sdp: String,
}
