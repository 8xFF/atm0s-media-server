use serde::{Deserialize, Serialize};

use crate::PayloadCodec;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPacketExtensions {
    pub audio_level: Option<i8>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPacket {
    pub codec: PayloadCodec,
    pub seq_no: u16,
    pub time: u32,
    pub marker: bool,
    pub ext_vals: MediaPacketExtensions,
    pub nackable: bool,
    pub payload: Vec<u8>,
}

impl MediaPacket {
    pub fn simple_audio(seq_no: u16, time: u32, payload: Vec<u8>) -> Self {
        Self {
            codec: PayloadCodec::Opus,
            seq_no,
            time,
            marker: false,
            ext_vals: MediaPacketExtensions { audio_level: None },
            nackable: false,
            payload,
        }
    }

    pub fn simple_video(codec: PayloadCodec, seq_no: u16, time: u32, payload: Vec<u8>) -> Self {
        Self {
            codec,
            seq_no,
            time,
            marker: false,
            ext_vals: MediaPacketExtensions { audio_level: None },
            nackable: false,
            payload,
        }
    }
}
