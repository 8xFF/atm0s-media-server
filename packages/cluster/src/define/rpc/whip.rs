use std::net::IpAddr;

use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

use crate::{MediaSessionToken, VerifyObject};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WhipConnectRequest {
    pub session_uuid: u64,
    pub ip_addr: IpAddr,
    pub user_agent: String,
    pub token: String,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
}

impl VerifyObject for WhipConnectRequest {
    fn verify(&self, verifier: &dyn crate::SessionTokenVerifier) -> Option<MediaSessionToken> {
        let token = verifier.verify_media_session(&self.token)?;
        if token.protocol != crate::rpc::general::MediaSessionProtocol::Whip {
            return None;
        }
        Some(token)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct WhipConnectResponse {
    pub conn_id: String,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
}
