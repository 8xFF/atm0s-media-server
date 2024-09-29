use derive_more::{
    derive::{Add, AddAssign, Deref, Display, Into},
    AsRef, From,
};
use serde::{Deserialize, Serialize};

use crate::{
    media::{MediaKind, MediaScaling},
    protobuf,
};

use super::{BitrateControlMode, PeerId};

///
/// TrackName type, we should use this type instead of direct String
/// This is useful when we can validate
///
/// TODO: validate with uuid type (maybe max 32 bytes + [a-z]_- )
///
#[derive(From, Into, Deref, AsRef, Debug, Display, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackName(String);

impl From<&str> for TrackName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(From, Deref, AsRef, Debug, Display, Add, AddAssign, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackPriority(u32);

impl TrackPriority {
    pub const fn build(v: u32) -> Self {
        TrackPriority(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMeta {
    pub kind: MediaKind,
    pub scaling: MediaScaling,
    pub control: BitrateControlMode,
    pub metadata: Option<String>,
}

impl TrackMeta {
    pub fn default_audio() -> Self {
        Self {
            kind: MediaKind::Audio,
            scaling: MediaScaling::None,
            control: BitrateControlMode::MaxBitrate,
            metadata: None,
        }
    }
}

///
/// TrackInfo will be used for broadcast to cluster
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub peer: PeerId,
    pub track: TrackName,
    pub meta: TrackMeta,
}

impl TrackInfo {
    pub fn simple_audio(peer: PeerId) -> Self {
        Self {
            peer,
            track: "audio_main".to_string().into(),
            meta: TrackMeta::default_audio(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("should ok")
    }

    pub fn deserialize(data: &[u8]) -> Option<TrackInfo> {
        bincode::deserialize::<Self>(data).ok()
    }
}

///
/// TrackSource is identify of a track in a room, this is used for attaching a source into a consumer.
/// A consumer can be: local track, audio_mixer ...
///
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct TrackSource {
    pub peer: PeerId,
    pub track: TrackName,
}

impl From<protobuf::shared::receiver::Source> for TrackSource {
    fn from(value: protobuf::shared::receiver::Source) -> Self {
        Self {
            peer: value.peer.into(),
            track: value.track.into(),
        }
    }
}
