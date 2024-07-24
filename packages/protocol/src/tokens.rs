use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct WhipToken {
    pub room: String,
    pub peer: String,
    pub record: bool,
    pub userdata: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WhepToken {
    pub room: String,
    pub peer: Option<String>,
    pub userdata: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WebrtcToken {
    pub room: Option<String>,
    pub peer: Option<String>,
    pub record: bool,
    pub userdata: Option<String>,
}
