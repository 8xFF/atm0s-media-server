use derivative::Derivative;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Audio,
    Video,
}

impl MediaKind {
    pub fn is_audio(&self) -> bool {
        matches!(self, MediaKind::Audio)
    }

    pub fn is_video(&self) -> bool {
        matches!(self, MediaKind::Video)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaScaling {
    None,
    Simulcat,
    Svc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaCodec {
    Opus,
    H264,
    Vp8,
    Vp9,
}

#[derive(Derivative, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct MediaPacket {
    pub pt: u8,
    pub ts: u32,
    pub seq: u64,
    pub marker: bool,
    pub nackable: bool,
    #[derivative(Debug = "ignore")]
    pub data: Vec<u8>,
}

impl MediaPacket {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("should ok")
    }

    pub fn deserialize(data: &[u8]) -> Option<MediaPacket> {
        bincode::deserialize::<Self>(data).ok()
    }
}
