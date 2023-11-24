use self::rid_history::RidHistory;
use str0m::{format::CodecConfig, media::Pt};
use transport::{H264Profile, MediaPacket, PayloadCodec, Vp9Profile};

use super::{
    mid_convert::rid_to_u16,
    pt_mapping::{LocalPayloadType, PtMapping},
};

mod bit_read;
mod h264;
mod rid_history;
mod vp8;
mod vp9;

#[derive(Default)]
pub struct RtpPacketConverter {
    rid_history: RidHistory,
    pt_mapping: PtMapping,
}

impl RtpPacketConverter {
    pub fn str0m_sync_codec_config(&mut self, config: &CodecConfig) {
        self.pt_mapping.str0m_sync_codec_config(config);
    }

    pub fn to_pkt(&mut self, rtp: str0m::rtp::RtpPacket) -> Option<MediaPacket> {
        let rid = self.rid_history.get(rtp.header.ext_vals.rid.map(|rid| rid_to_u16(&rid)), *(&rtp.header.ssrc as &u32));

        let local_pt = self.pt_mapping.to_local(*rtp.header.payload_type);
        let (codec, nackable) = match local_pt {
            LocalPayloadType::Opus => Some((PayloadCodec::Opus, false)),
            LocalPayloadType::Vp8 => {
                let (is_key, sim) = vp8::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::Vp8(is_key, sim), true))
            }
            LocalPayloadType::Vp9P0 => {
                let (is_key, svc) = vp9::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::Vp9(is_key, Vp9Profile::P0, svc), true))
            }
            LocalPayloadType::Vp9P2 => {
                let (is_key, svc) = vp9::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::Vp9(is_key, Vp9Profile::P2, svc), true))
            }
            LocalPayloadType::H264_42001fNon => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::H264(is_key, H264Profile::P42001fNonInterleaved, sim), true))
            }
            LocalPayloadType::H264_42001fSingle => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::H264(is_key, H264Profile::P42001fSingleNal, sim), true))
            }
            LocalPayloadType::H264_42e01fNon => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::H264(is_key, H264Profile::P42e01fNonInterleaved, sim), true))
            }
            LocalPayloadType::H264_42e01fSingle => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::H264(is_key, H264Profile::P42e01fSingleNal, sim), true))
            }
            LocalPayloadType::H264_4d001fNon => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::H264(is_key, H264Profile::P4d001fNonInterleaved, sim), true))
            }
            LocalPayloadType::H264_4d001fSingle => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::H264(is_key, H264Profile::P4d001fSingleNal, sim), true))
            }
            LocalPayloadType::H264_64001fNon => {
                let (is_key, sim) = h264::payload_parse(&rtp.payload, rid);
                Some((PayloadCodec::H264(is_key, H264Profile::P64001fNonInterleaved, sim), true))
            }
            LocalPayloadType::Unknown => {
                log::warn!("unknown payload type {}", rtp.header.payload_type);
                None
            }
        }?;
        Some(MediaPacket {
            codec,
            seq_no: rtp.header.sequence_number,
            time: rtp.header.timestamp,
            marker: rtp.header.marker,
            // ext_vals: MediaPacketExtensions {
            //     abs_send_time: rtp.header.ext_vals.abs_send_time.map(|t| (t.number(), t.denom())),
            //     transport_cc: rtp.header.ext_vals.transport_cc,
            // },
            nackable: nackable,
            payload: rtp.payload,
        })
    }
}

#[derive(Default)]
pub struct MediaPacketConvert {
    pt_mapping: PtMapping,
}

impl MediaPacketConvert {
    pub fn str0m_sync_codec_config(&mut self, config: &CodecConfig) {
        self.pt_mapping.str0m_sync_codec_config(config);
    }

    pub fn to_pt(&self, media: &MediaPacket) -> Pt {
        let local_pt = match &media.codec {
            PayloadCodec::Vp8(_, _) => LocalPayloadType::Vp8,
            PayloadCodec::Vp9(_, profile, _) => match profile {
                Vp9Profile::P0 => LocalPayloadType::Vp9P0,
                Vp9Profile::P2 => LocalPayloadType::Vp9P2,
            },
            PayloadCodec::H264(_, profile, _) => match profile {
                H264Profile::P42001fNonInterleaved => LocalPayloadType::H264_42001fNon,
                H264Profile::P42001fSingleNal => LocalPayloadType::H264_42001fSingle,
                H264Profile::P42e01fNonInterleaved => LocalPayloadType::H264_42e01fNon,
                H264Profile::P42e01fSingleNal => LocalPayloadType::H264_42e01fSingle,
                H264Profile::P4d001fNonInterleaved => LocalPayloadType::H264_4d001fNon,
                H264Profile::P4d001fSingleNal => LocalPayloadType::H264_4d001fSingle,
                H264Profile::P64001fNonInterleaved => LocalPayloadType::H264_64001fNon,
            },
            PayloadCodec::Opus => LocalPayloadType::Opus,
        };

        self.pt_mapping.to_remote(local_pt).into()
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
