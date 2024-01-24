use std::net::IpAddr;

use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

use crate::VerifyObject;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WhepConnectRequest {
    pub session_uuid: u64,
    pub ip_addr: IpAddr,
    pub user_agent: String,
    pub token: String,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
}

impl VerifyObject for WhepConnectRequest {
    fn verify(&self, verifier: &dyn crate::SessionTokenVerifier) -> Option<crate::MediaSessionToken> {
        let token = verifier.verify_media_session(&self.token)?;
        if token.protocol != crate::rpc::general::MediaSessionProtocol::Whep {
            return None;
        }
        Some(token)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct WhepConnectResponse {
    pub conn_id: String,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
}
