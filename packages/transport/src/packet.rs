use crate::PayloadCodec;

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct MediaPacketExtensions {
//     pub abs_send_time: Option<(i64, i64)>,
//     pub transport_cc: Option<u16>, // (buf[0] << 8) | buf[1];
// }

#[derive(Debug, Clone, PartialEq, Eq)]
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
