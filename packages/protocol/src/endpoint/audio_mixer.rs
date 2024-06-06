use serde::{Deserialize, Serialize};

use crate::{
    protobuf::features::mixer::Mode,
    transport::{LocalTrackId, RemoteTrackId},
};

use super::{PeerHashCode, PeerId, TrackName, TrackSource};

#[derive(Debug, PartialEq, Eq)]
pub enum AudioMixerMode {
    Auto,
    Manual,
}

impl From<Mode> for AudioMixerMode {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Auto => AudioMixerMode::Auto,
            Mode::Manual => AudioMixerMode::Manual,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AudioMixerConfig {
    pub mode: AudioMixerMode,
    pub outputs: Vec<LocalTrackId>,
    pub sources: Vec<TrackSource>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMixerPkt {
    pub slot: u8,
    pub peer: PeerHashCode,
    pub track: RemoteTrackId,
    pub audio_level: Option<i8>,
    pub source: Option<(PeerId, TrackName)>,
    pub ts: u32,
    pub seq: u16,
    pub opus_payload: Vec<u8>,
}

impl AudioMixerPkt {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("should ok")
    }

    pub fn deserialize(data: &[u8]) -> Option<AudioMixerPkt> {
        bincode::deserialize::<Self>(data).ok()
    }
}
