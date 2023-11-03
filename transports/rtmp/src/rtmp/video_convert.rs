use std::collections::VecDeque;

use bytes::{Bytes, BytesMut};
use rtp::codecs::h264::H264Payloader;
use rtp::packetizer::Payloader;
use transport::{H264Profile, MediaPacket, PayloadCodec};
use xflv::demuxer::FlvVideoTagDemuxer;

pub struct RtmpH264ToMediaPacketH264 {
    demuxer: FlvVideoTagDemuxer,
    outputs: VecDeque<MediaPacket>,
    seq_no: u16,
    rtp_packetizer: H264Payloader,
}

impl RtmpH264ToMediaPacketH264 {
    pub fn new() -> Self {
        Self {
            demuxer: FlvVideoTagDemuxer::new(),
            outputs: VecDeque::new(),
            seq_no: 0,
            rtp_packetizer: H264Payloader::default(),
        }
    }

    pub fn push(&mut self, data: Bytes, ts_ms: u32) -> Option<()> {
        let data = BytesMut::from(&data as &[u8]);
        if let Some(frame) = self.demuxer.demux(ts_ms, data).ok()? {
            log::debug!("on h264 flvdemux frame type: {} {} {}", frame.frame_type, self.seq_no, (ts_ms * 90) as u32);

            let pkts = self.rtp_packetizer.payload(1200, &frame.data.freeze()).ok()?;
            for pkt in pkts {
                let codec = PayloadCodec::H264(frame.frame_type == 1, H264Profile::P42001fNonInterleaved, None);
                let mut media = MediaPacket::simple_video(codec, self.seq_no, ts_ms * 90, pkt.to_vec());
                media.nackable = true;
                self.outputs.push_back(media);
                self.seq_no += 1;
            }
            if let Some(pkt) = self.outputs.back_mut() {
                pkt.marker = true;
            }
        }
        Some(())
    }

    pub fn pop(&mut self) -> Option<MediaPacket> {
        self.outputs.pop_front()
    }
}
