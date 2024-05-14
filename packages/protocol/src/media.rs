use derivative::Derivative;
use derive_more::From;
use serde::{Deserialize, Serialize};

use crate::protobuf::shared::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
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

    pub fn sample_rate(&self) -> u64 {
        if self.is_audio() {
            48000
        } else {
            90000
        }
    }
}

impl From<Kind> for MediaKind {
    fn from(value: Kind) -> Self {
        match value {
            Kind::Audio => Self::Audio,
            Kind::Video => Self::Video,
        }
    }
}

impl From<MediaKind> for Kind {
    fn from(value: MediaKind) -> Self {
        match value {
            MediaKind::Audio => Self::Audio,
            MediaKind::Video => Self::Video,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaSimple {
    pub key: bool,
    pub bitrate: Option<u16>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
pub struct MediaLayerBitrate([Option<u16>; 3]);

impl MediaLayerBitrate {
    pub fn new(data: &[u16; 3]) -> Self {
        Self([Some(data[0]), Some(data[1]), Some(data[2])])
    }

    pub fn set_layer(&mut self, index: usize, bitrate: u16) {
        self.0[index] = Some(bitrate);
    }

    pub fn get_layer(&mut self, index: usize) -> Option<u16> {
        self.0[index]
    }

    pub fn number_temporals(&self) -> u8 {
        if self.0[0].is_none() {
            return 0;
        }

        if self.0[1].is_none() {
            return 1;
        }

        if self.0[2].is_none() {
            return 2;
        }

        return 3;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLayerSelection {
    pub spatial: u8,
    pub temporal: u8,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
pub struct MediaLayersBitrate([Option<MediaLayerBitrate>; 3]);

impl MediaLayersBitrate {
    pub fn default_sim() -> Self {
        Self([Some(MediaLayerBitrate([Some(81), None, None])), None, None])
    }

    pub fn set_layer(&mut self, index: usize, layer: MediaLayerBitrate) {
        self.0[index] = Some(layer);
    }

    pub fn has_layer(&self, index: usize) -> bool {
        self.0[index].is_some()
    }

    pub fn number_layers(&self) -> u8 {
        if self.0[0].is_none() {
            return 0;
        }

        if self.0[1].is_none() {
            return 1;
        }

        if self.0[2].is_none() {
            return 2;
        }

        return 3;
    }

    pub fn number_temporals(&self) -> u8 {
        self.0[0].as_ref().map(|l| l.number_temporals()).unwrap_or(0)
    }

    /// Select best layer for target bitrate
    /// TODO: return None if target_bitrate cannot provide stable connection
    pub fn select_layer(&self, target_bitrate_kbps: u16, max_spatial: u8, max_temporal: u8) -> Option<MediaLayerSelection> {
        let mut spatial = 0;
        let mut temporal = 0;
        for i in 0..=max_spatial.min(2) {
            if let Some(layer) = &self.0[i as usize] {
                for j in 0..=max_temporal.min(2) {
                    if let Some(bitrate) = layer.0[j as usize] {
                        if target_bitrate_kbps >= bitrate {
                            spatial = i;
                            temporal = j;
                        }
                    }
                }
            }
        }
        Some(MediaLayerSelection { spatial, temporal })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum Vp9Profile {
    P0,
    P2,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum H264Profile {
    P42001fNonInterleaved,
    P42001fSingleNal,
    P42e01fNonInterleaved,
    P42e01fSingleNal,
    P4d001fNonInterleaved,
    P4d001fSingleNal,
    P64001fNonInterleaved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct H264Sim {
    pub spatial: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vp8Sim {
    pub picture_id: Option<u16>,
    pub tl0_pic_idx: Option<u8>,
    pub spatial: u8,
    pub temporal: u8,
    pub layer_sync: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vp9Svc {
    pub spatial: u8,
    pub temporal: u8,
    pub begin_frame: bool,
    pub end_frame: bool,
    pub spatial_layers: Option<u8>,
    pub picture_id: Option<u16>,
    pub switching_point: bool,
    pub predicted_frame: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaScaling {
    None,
    Simulcast,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum MediaCodec {
    Opus,
    Vp8,
    H264(H264Profile),
    Vp9(Vp9Profile),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaMeta {
    Opus { audio_level: Option<i8> },
    H264 { key: bool, profile: H264Profile, sim: Option<H264Sim> },
    Vp8 { key: bool, sim: Option<Vp8Sim> },
    Vp9 { key: bool, profile: Vp9Profile, svc: Option<Vp9Svc> },
}

impl MediaMeta {
    pub fn is_audio(&self) -> bool {
        matches!(self, MediaMeta::Opus { .. })
    }

    pub fn is_video_key(&self) -> bool {
        match self {
            Self::H264 { key, .. } | Self::Vp8 { key, .. } | Self::Vp9 { key, .. } => *key,
            Self::Opus { .. } => false,
        }
    }

    pub fn codec(&self) -> MediaCodec {
        match self {
            Self::Opus { .. } => MediaCodec::Opus,
            Self::H264 { profile, .. } => MediaCodec::H264(*profile),
            Self::Vp8 { .. } => MediaCodec::Vp8,
            Self::Vp9 { profile, .. } => MediaCodec::Vp9(*profile),
        }
    }
}

#[derive(Derivative, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct MediaPacket {
    pub ts: u32,
    pub seq: u16,
    pub marker: bool,
    pub nackable: bool,
    pub layers: Option<MediaLayersBitrate>,
    pub meta: MediaMeta,
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
