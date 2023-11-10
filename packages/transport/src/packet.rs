use serde::{Deserialize, Serialize};

use crate::PayloadCodec;

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct MediaPacketExtensions {
//     pub abs_send_time: Option<(i64, i64)>,
//     pub transport_cc: Option<u16>, // (buf[0] << 8) | buf[1];
// }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPacket {
    pub codec: PayloadCodec,
    pub seq_no: u16,
    pub time: u32,
    pub marker: bool,
    // pub ext_vals: MediaPacketExtensions,
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
            // ext_vals: MediaPacketExtensions {
            //     abs_send_time: None,
            //     transport_cc: None,
            // },
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
            // ext_vals: MediaPacketExtensions {
            //     abs_send_time: None,
            //     transport_cc: None,
            // },
            nackable: false,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_audio_packet() {
        let seq_no = 1;
        let time = 1234;
        let payload = vec![0x01, 0x02, 0x03];
        let packet = MediaPacket::simple_audio(seq_no, time, payload.clone());

        assert_eq!(packet.codec, PayloadCodec::Opus);
        assert_eq!(packet.seq_no, seq_no);
        assert_eq!(packet.time, time);
        assert_eq!(packet.marker, false);
        assert_eq!(packet.nackable, false);
        assert_eq!(packet.payload, payload);
    }

    #[test]
    fn test_simple_video_packet() {
        let codec = PayloadCodec::Vp8(false, None);
        let seq_no = 2;
        let time = 5678;
        let payload = vec![0x04, 0x05, 0x06];
        let packet = MediaPacket::simple_video(codec.clone(), seq_no, time, payload.clone());

        assert_eq!(packet.codec, codec);
        assert_eq!(packet.seq_no, seq_no);
        assert_eq!(packet.time, time);
        assert_eq!(packet.marker, false);
        assert_eq!(packet.nackable, false);
        assert_eq!(packet.payload, payload);
    }
}
