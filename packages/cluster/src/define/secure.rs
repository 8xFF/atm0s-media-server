use atm0s_sdn::NodeId;
use serde::{Deserialize, Serialize};

use crate::rpc::general::MediaSessionProtocol;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaSessionToken {
    pub room: Option<String>,
    pub peer: Option<String>,
    pub protocol: MediaSessionProtocol,
    pub publish: bool,
    pub subscribe: bool,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaConnId {
    pub node_id: NodeId,
    pub conn_id: u64,
}

pub trait SessionTokenSigner {
    fn sign_media_session(&self, token: &MediaSessionToken) -> String;
    fn sign_conn_id(&self, conn_id: &MediaConnId) -> String;
}

pub trait SessionTokenVerifier {
    fn verify_media_session(&self, token: &str) -> Option<MediaSessionToken>;
    fn verify_conn_id(&self, token: &str) -> Option<MediaConnId>;
}

pub trait VerifyObject {
    fn verify(&self, verifier: &dyn SessionTokenVerifier) -> Option<MediaSessionToken>;
}
