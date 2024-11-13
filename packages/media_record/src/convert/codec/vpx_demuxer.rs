use bytes::{Bytes, BytesMut};
use media_server_protocol::media::MediaPacket;
use rtp::packetizer::Depacketizer;

pub struct VpxDemuxer {
    seen_key_frame: bool,
    current_frame: Option<(bool, BytesMut)>,
}

impl Default for VpxDemuxer {
    fn default() -> Self {
        Self::new()
    }
}

impl VpxDemuxer {
    pub fn new() -> Self {
        Self {
            seen_key_frame: false,
            current_frame: None,
        }
    }

    pub fn push(&mut self, rtp: MediaPacket) -> Option<(bool, Bytes)> {
        let data = Bytes::from(rtp.data);
        let (mut depacketizer, is_key_frame) = match rtp.meta {
            media_server_protocol::media::MediaMeta::Opus { .. } => panic!("wrong codec"),
            media_server_protocol::media::MediaMeta::H264 { .. } => panic!("wrong codec"),
            media_server_protocol::media::MediaMeta::Vp8 { key, sim, rotation } => {
                if let Some(sim) = sim {
                    if sim.spatial != 0 {
                        //TODO: how to get maximum quality
                        return None;
                    }
                }
                if let Some(_rotation) = rotation {
                    //TODO: process rotation
                }
                (Box::new(rtp::codecs::vp8::Vp8Packet::default()) as Box<dyn Depacketizer>, key)
            }
            media_server_protocol::media::MediaMeta::Vp9 { key, .. } => (Box::new(rtp::codecs::vp9::Vp9Packet::default()) as Box<dyn Depacketizer>, key),
        };
        let payload = depacketizer.depacketize(&data).unwrap();
        if !self.seen_key_frame && !is_key_frame {
            log::info!("reject");
            return None;
        }

        self.seen_key_frame = true;
        if let Some((_is_key, current_frame)) = &mut self.current_frame {
            current_frame.extend(payload);
        } else {
            let mut current_frame = BytesMut::new();
            current_frame.extend(payload);
            self.current_frame = Some((is_key_frame, current_frame));
        };

        if !rtp.marker {
            return None;
        }

        let (is_key_frame, frame) = self.current_frame.take()?;
        Some((is_key_frame, frame.freeze()))
    }
}
