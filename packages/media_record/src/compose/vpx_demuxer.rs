use bytes::{Bytes, BytesMut};
use media_server_protocol::media::MediaPacket;
use rtp::packetizer::Depacketizer;

pub struct VpxDemuxer {
    count: u64,
    seen_key_frame: bool,
    current_frame: Option<(bool, BytesMut)>,
}

impl VpxDemuxer {
    pub fn new() -> Self {
        Self {
            count: 0,
            seen_key_frame: false,
            current_frame: None,
        }
    }

    pub fn push(&mut self, is_key_frame: bool, rtp: MediaPacket) -> Option<(bool, u32, Bytes)> {
        // log::info!("{} {} {is_key_frame} {} {}", rtp.seq, rtp.ts, rtp.marker, rtp.data.len());
        let data = Bytes::from(rtp.data);
        let mut depacketizer = rtp::codecs::vp8::Vp8Packet::default();
        let payload = depacketizer.depacketize(&data).unwrap();
        if !self.seen_key_frame && !is_key_frame {
            log::info!("reject");
            panic!("");
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
        Some((is_key_frame, rtp.ts, frame.freeze()))
    }
}
