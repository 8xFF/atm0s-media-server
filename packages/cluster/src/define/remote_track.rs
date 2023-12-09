use serde::{Deserialize, Serialize};
use transport::{MediaKind, MediaPacket, RequestKeyframeKind};

use crate::ClusterTrackName;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ClusterTrackStats {
    Single { bitrate: u32 },
    Simulcast { bitrate: u32, layers: [[u32; 3]; 3] },
    Svc { bitrate: u32, layers: [[u32; 3]; 3] },
}

impl ClusterTrackStats {
    pub fn consumer_bitrate_scale(&self) -> f32 {
        match self {
            ClusterTrackStats::Single { .. } => 1.0,
            ClusterTrackStats::Simulcast { .. } => 1.5,
            ClusterTrackStats::Svc { .. } => 1.0,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
pub enum ClusterTrackStatus {
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "reconnecting")]
    Reconnecting,
    #[serde(rename = "disconnected")]
    Disconnected,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum ClusterTrackScalingType {
    Single,
    Simulcast,
    Svc,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ClusterTrackMeta {
    pub kind: MediaKind,
    pub scaling: ClusterTrackScalingType,
    pub layers: Vec<u32>,
    pub status: ClusterTrackStatus,
    pub active: bool,
    pub label: Option<String>,
}

impl ClusterTrackMeta {
    pub fn default_audio() -> Self {
        Self {
            kind: MediaKind::Audio,
            scaling: ClusterTrackScalingType::Single,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            active: true,
            label: None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClusterRemoteTrackIncomingEvent {
    RequestKeyFrame(RequestKeyframeKind),
    RequestLimitBitrate(u32),
}

#[derive(PartialEq, Eq, Debug)]
pub enum ClusterRemoteTrackOutgoingEvent {
    TrackAdded(ClusterTrackName, ClusterTrackMeta),
    TrackMedia(MediaPacket),
    TrackStats(ClusterTrackStats),
    TrackRemoved(ClusterTrackName),
}
