use derivative::Derivative;
use derive_more::From;
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

    pub fn sample_rate(&self) -> u64 {
        if self.is_audio() {
            48000
        } else {
            90000
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
    pub fn set_layer(&mut self, index: usize, bitrate: u16) {
        self.0[index] = Some(bitrate);
    }

    pub fn get_layer(&mut self, index: usize) -> Option<u16> {
        self.0[index]
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
pub struct MediaLayersBitrate([Option<MediaLayerBitrate>; 3]);

impl MediaLayersBitrate {
    pub fn set_layer(&mut self, index: usize, layer: MediaLayerBitrate) {
        self.0[index] = Some(layer);
    }

    pub fn select_layer(&self, target_bitrate_kbps: u16) -> (u8, u8) {
        let mut target_spatial = 0;
        let mut target_temporal = 0;
        for i in 0..3 {
            if let Some(spatial) = &self.0[i] {
                for j in 0..3 {
                    if let Some(bitrate) = spatial.0[j] {
                        if target_bitrate_kbps >= bitrate {
                            target_spatial = i as u8;
                            target_temporal = j as u8;
                        }
                    }
                }
            }
        }
        (target_spatial, target_temporal)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Vp9 { key: bool, profile: Vp9Profile, sim: Option<Vp9Svc> },
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
