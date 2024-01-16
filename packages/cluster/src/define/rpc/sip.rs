use poem_openapi::{Enum, Object};
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipOutgoingAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipOutgoingInviteClientRequest {
    pub room_id: String,
    pub dest_session_id: String,
    pub from_number: String,
    pub server_alias: Option<String>,
    pub hook_uri: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipOutgoingInviteServerRequest {
    pub room_id: String,
    pub dest_addr: String,
    pub dest_auth: SipOutgoingAuth,
    pub from_number: String,
    pub to_number: String,
    pub server_alias: String,
    pub hook_uri: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipOutgoingInviteResponse {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingAuthRequest {
    pub username: String,
    pub realm: String,
    pub session_id: String,
}

/// In SIP, the digest authentication is:
/// HA1=MD5(username:realm:password)
/// HA2=MD5(method:digestURI)
/// response=MD5(HA1:nonce:HA2)
///
/// Therefore we need to return HA1 to the sip server
#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingAuthResponse {
    pub success: bool,
    pub ha1: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingRegisterRequest {
    pub username: String,
    pub realm: String,
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingRegisterResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingUnregisterRequest {
    pub username: String,
    pub realm: String,
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingUnregisterResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingInviteRequest {
    pub source: String,
    pub username: Option<String>,
    pub from_number: String,
    pub to_number: String,
    pub conn_id: String,
}

#[derive(Debug, Serialize, Deserialize, Enum, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub enum SipIncomingInviteStrategy {
    Reject,
    Accept,
    WaitOtherPeers,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct SipIncomingInviteResponse {
    pub strategy: SipIncomingInviteStrategy,
    pub room_id: Option<String>,
}
