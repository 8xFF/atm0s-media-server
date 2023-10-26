use self::rid_history::RidHistory;
use str0m::media::Pt;
use transport::{H264Profile, MediaPacket, MediaPacketExtensions, PayloadCodec, Vp9Profile};

use super::utils::rid_to_u16;

mod bit_read;
mod h264;
mod rid_history;
mod vp8;
mod vp9;

const PAYLOAD_TYPE_VP8: u8 = 96;
// const PAYLOAD_TYPE_VP8_RTX: u8 = 97;
const PAYLOAD_TYPE_VP9_P0: u8 = 98;
// const PAYLOAD_TYPE_VP9_P0_RTX: u8 = 99;
const PAYLOAD_TYPE_VP9_P2: u8 = 100;
// const PAYLOAD_TYPE_VP9_P2_RTX: u8 = 101;
const PAYLOAD_TYPE_H264_42001F_NON: u8 = 121;
// const PAYLOAD_TYPE_H264_42001F_NON_RTX: u8 = 103;
const PAYLOAD_TYPE_H264_42001F_SINGLE: u8 = 125;
// const PAYLOAD_TYPE_H264_42001F_SINGLE_RTX: u8 = 107;
const PAYLOAD_TYPE_H264_42E01F_NON: u8 = 108;
// const PAYLOAD_TYPE_H264_42E01F_NON_RTX: u8 = 109;
const PAYLOAD_TYPE_H264_42E01F_SINGLE: u8 = 124;
// const PAYLOAD_TYPE_H264_42E01F_SINGLE_RTX: u8 = 120;
const PAYLOAD_TYPE_H264_4D001F_NON: u8 = 123;
// const PAYLOAD_TYPE_H264_4D001F_NON_RTX: u8 = 119;
const PAYLOAD_TYPE_H264_4D001F_SINGLE: u8 = 35;
// const PAYLOAD_TYPE_H264_4D001F_SINGLE_RTX: u8 = 36;
const PAYLOAD_TYPE_H264_64001F_NON: u8 = 114;
// const PAYLOAD_TYPE_H264_64001F_NON_RTX: u8 = 115;
const PAYLOAD_TYPE_OPUS: u8 = 111;

#[derive(Default)]
pub struct RtpPacketConverter {
    rid_history: RidHistory,
}

impl RtpPacketConverter {
    pub fn to_pkt(&mut self, rtp: str0m::rtp::RtpPacket) -> Option<MediaPacket> {
        let rid = self.rid_history.get(rtp.header.ext_vals.rid.map(|rid| rid_to_u16(&rid)), *(&rtp.header.ssrc as &u32));

        let codec = match *rtp.header.payload_type {
            PAYLOAD_TYPE_OPUS => Some(PayloadCodec::Opus),
            PAYLOAD_TYPE_VP8 => {
                let (is_key, sim) = vp8::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::Vp8(is_key, sim))
            }
            PAYLOAD_TYPE_VP9_P0 => {
                let (is_key, svc) = vp9::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::Vp9(is_key, Vp9Profile::P0, svc))
            }
            PAYLOAD_TYPE_VP9_P2 => {
                let (is_key, svc) = vp9::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::Vp9(is_key, Vp9Profile::P2, svc))
            }
            PAYLOAD_TYPE_H264_42001F_NON => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::H264(is_key, H264Profile::P42001fNonInterleaved, sim))
            }
            PAYLOAD_TYPE_H264_42001F_SINGLE => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::H264(is_key, H264Profile::P42001fSingleNal, sim))
            }
            PAYLOAD_TYPE_H264_42E01F_NON => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::H264(is_key, H264Profile::P42e01fNonInterleaved, sim))
            }
            PAYLOAD_TYPE_H264_42E01F_SINGLE => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::H264(is_key, H264Profile::P42e01fSingleNal, sim))
            }
            PAYLOAD_TYPE_H264_4D001F_NON => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::H264(is_key, H264Profile::P4d001fNonInterleaved, sim))
            }
            PAYLOAD_TYPE_H264_4D001F_SINGLE => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::H264(is_key, H264Profile::P4d001fSingleNal, sim))
            }
            PAYLOAD_TYPE_H264_64001F_NON => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some(PayloadCodec::H264(is_key, H264Profile::P64001fNonInterleaved, sim))
            }
            _ => None,
        }?;
        Some(MediaPacket {
            codec,
            seq_no: rtp.header.sequence_number,
            time: rtp.header.timestamp,
            marker: rtp.header.marker,
            ext_vals: MediaPacketExtensions {
                abs_send_time: rtp.header.ext_vals.abs_send_time.map(|t| (t.numer(), t.denom())),
                transport_cc: rtp.header.ext_vals.transport_cc,
            },
            nackable: true,
            payload: rtp.payload,
        })
    }
}

#[derive(Default)]
pub struct MediaPacketConvert {}

impl MediaPacketConvert {
    pub fn to_pt(&self, media: &MediaPacket) -> Pt {
        let pt = match &media.codec {
            PayloadCodec::Vp8(_, _) => PAYLOAD_TYPE_VP8,
            PayloadCodec::Vp9(_, profile, _) => match profile {
                Vp9Profile::P0 => PAYLOAD_TYPE_VP9_P0,
                Vp9Profile::P2 => PAYLOAD_TYPE_VP9_P2,
            },
            PayloadCodec::H264(_, profile, _) => match profile {
                H264Profile::P42001fNonInterleaved => PAYLOAD_TYPE_H264_42001F_NON,
                H264Profile::P42001fSingleNal => PAYLOAD_TYPE_H264_42001F_SINGLE,
                H264Profile::P42e01fNonInterleaved => PAYLOAD_TYPE_H264_42E01F_NON,
                H264Profile::P42e01fSingleNal => PAYLOAD_TYPE_H264_42E01F_SINGLE,
                H264Profile::P4d001fNonInterleaved => PAYLOAD_TYPE_H264_4D001F_NON,
                H264Profile::P4d001fSingleNal => PAYLOAD_TYPE_H264_4D001F_SINGLE,
                H264Profile::P64001fNonInterleaved => PAYLOAD_TYPE_H264_64001F_NON,
            },
            PayloadCodec::Opus => PAYLOAD_TYPE_OPUS,
        };

        pt.into()
    }

    pub fn rewrite_codec(&self, media: &mut MediaPacket) {
        match &media.codec {
            PayloadCodec::Vp8(_, Some(sim)) => {
                vp8::payload_rewrite(&mut media.payload, sim);
            }
            PayloadCodec::Vp9(_, _, Some(svc)) => {
                vp9::payload_rewrite(&mut media.payload, svc);
            }
            _ => {}
        }
    }
}
