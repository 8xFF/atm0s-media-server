use poem_openapi::Object;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct WhepConnectRequest {
    pub token: String,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct WhepConnectResponse {
    pub conn_id: String,
    pub sdp: Option<String>,
    pub compressed_sdp: Option<Vec<u8>>,
}
