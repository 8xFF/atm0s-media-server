use std::fmt::Debug;

use crate::{ClusterEndpointPublishScope, ClusterEndpointSubscribeScope, MediaSessionToken, VerifyObject};

use super::{
    super::media::{BitrateControlMode, MixMinusAudioMode, PayloadType},
    general::RemoteAddr,
};
use poem_openapi::Object;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};
use transport::MediaKind;

#[derive(Serialize, Deserialize, Debug, Object, PartialEq, Eq, Clone)]
pub struct WebrtcConnectRequestReceivers {
    pub audio: u8,
    pub video: u8,
}

#[derive(Serialize, Deserialize, Debug, Object, PartialEq, Eq, Clone)]
pub struct WebrtcConnectRequestSender {
    pub kind: MediaKind,
    pub name: String,
    pub uuid: String,
    pub label: String,
    pub screen: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WebrtcConnectRequest {
    #[oai(skip)]
    pub session_uuid: u64,
    #[oai(skip)]
    pub ip_addr: RemoteAddr,
    #[oai(skip)]
    pub user_agent: String,
    pub version: Option<String>,
    pub room: String,
    pub peer: String,
    #[oai(default = "ClusterEndpointSubscribeScope::default")]
    pub sub_scope: ClusterEndpointSubscribeScope,
    #[oai(default = "ClusterEndpointPublishScope::default")]
    pub pub_scope: ClusterEndpointPublishScope,
    pub token: String,
    #[oai(default = "MixMinusAudioMode::default")]
    pub mix_minus_audio: MixMinusAudioMode,
    pub join_now: Option<bool>,
    pub codecs: Option<Vec<PayloadType>>,
    pub receivers: WebrtcConnectRequestReceivers,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
    pub senders: Vec<WebrtcConnectRequestSender>,
    #[oai(default = "BitrateControlMode::default")]
    pub remote_bitrate_control_mode: BitrateControlMode,
}

impl VerifyObject for WebrtcConnectRequest {
    fn verify(&self, verifier: &dyn crate::SessionTokenVerifier) -> Option<MediaSessionToken> {
        let token = verifier.verify_media_session(&self.token)?;
        if token.protocol != crate::rpc::general::MediaSessionProtocol::Webrtc {
            return None;
        }
        if let Some(room) = &token.room {
            if !room.eq(&self.room) {
                return None;
            }
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WebrtcPatchRequest {
    pub conn_id: String,
    pub sdp: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct WebrtcPatchResponse {
    pub ice_restart_sdp: Option<String>,
}
