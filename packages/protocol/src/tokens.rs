use serde::{Deserialize, Serialize};

pub const WHIP_TOKEN: &str = "whip";
pub const WHEP_TOKEN: &str = "whep";
pub const WEBRTC_TOKEN: &str = "webrtc";
pub const RTPENGINE_TOKEN: &str = "rtpengine";

#[derive(Serialize, Deserialize, Debug)]
pub struct WhipToken {
    pub app: Option<String>,
    pub room: String,
    pub peer: String,
    pub record: bool,
    pub extra_data: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WhepToken {
    pub app: Option<String>,
    pub room: String,
    pub peer: Option<String>,
    pub extra_data: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WebrtcToken {
    pub app: Option<String>,
    pub room: Option<String>,
    pub peer: Option<String>,
    pub record: bool,
    pub extra_data: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RtpEngineToken {
    pub app: Option<String>,
    pub room: String,
    pub peer: String,
    pub record: bool,
    pub extra_data: Option<String>,
}
