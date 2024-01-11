use poem_openapi::Object;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8, Clone)]
pub struct MediaEndpointCloseRequest {
    pub conn_id: String,
}

#[derive(Debug, Serialize, Deserialize, Object, PartialEq, Eq, IntoVecU8, TryFromSliceU8)]
pub struct MediaEndpointCloseResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum MediaSessionProtocol {
    Whip,
    Whep,
    Webrtc,
    Rtmp,
    Sip,
}
