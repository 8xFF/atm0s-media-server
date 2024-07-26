use media_server_protocol::media::{MediaCodec, MediaMeta, MediaPacket};
use rtp::packet::Packet;

use crate::sdp::{Codec, CodecSpec, RtpCodecConfig};

#[derive(Default)]
pub struct MediaConverter {
    map: smallmap::Map<u8, MediaCodec>,
}

impl MediaConverter {
    pub fn convert(&self, rtp: Packet) -> Option<MediaPacket> {
        let codec = self.remote_pt_to_codec(rtp.header.payload_type)?;
        let (nackable, layers, meta) = match codec {
            MediaCodec::Opus => (false, None, MediaMeta::Opus { audio_level: Some(0) }),
            _ => return None,
        };
        Some(MediaPacket {
            ts: rtp.header.timestamp,
            seq: rtp.header.sequence_number,
            marker: rtp.header.marker,
            nackable,
            layers,
            meta,
            data: rtp.payload.to_vec(),
        })
    }

    fn remote_pt_to_codec(&self, pt: u8) -> Option<MediaCodec> {
        self.map.get(&pt).cloned()
    }

    pub fn set_config(&mut self, cfg: &RtpCodecConfig) {
        for param in cfg.params.iter() {
            if let Some(codec) = convert_codec(&param.spec) {
                self.map.insert(param.payload_type, codec);
            }
        }
    }
}

fn convert_codec(spec: &CodecSpec) -> Option<MediaCodec> {
    match spec.codec {
        Codec::Opus { .. } => Some(MediaCodec::Opus),
        _ => None,
    }
}
