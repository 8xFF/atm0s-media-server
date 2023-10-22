use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum MediaKind {
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
}
