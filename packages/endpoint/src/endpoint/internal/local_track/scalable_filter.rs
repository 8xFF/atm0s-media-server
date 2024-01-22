use media_utils::{SeqRewrite, TsRewrite};
use transport::{MediaPacket, PayloadCodec};

use crate::endpoint::internal::bitrate_allocator::LocalTrackTarget;

use self::{h264_sim::H264SimulcastFilter, video_single::VideoSingleFilter, vp8_sim::Vp8SimulcastFilter, vp9_svc::Vp9SvcFilter};

mod h264_sim;
mod video_single;
mod vp8_sim;
mod vp9_svc;

const SEQ_MAX: u64 = 1 << 16;
const TS_MAX: u64 = 1 << 32;

pub type MediaSeqRewrite = SeqRewrite<SEQ_MAX, 1000>;
pub type MediaTsRewrite = TsRewrite<TS_MAX, 10>;

#[derive(Debug, PartialEq, Eq)]
pub enum FilterResult {
    /// When this packet should be send
    Send,
    /// When this packet same stream with sending stream but it need to drop. This is used for sync seq rewrite
    Drop,
    /// Just reject this packet
    Reject,
}

trait ScalableFilter: Send + Sync {
    fn pause(&mut self);

    fn resume(&mut self);

    /// Configure the target layer to send to the remote peer. If return true => should send a key frame.
    fn set_target_layer(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool;

    /// Returns true if the packet should be sent to the remote peer.
    /// This is used to implement simulcast and SVC.
    /// The packet is modified in place to remove layers that should not be sent.
    /// Also return stream just changed or not, in case of just changed => need reinit seq and ts rewriter
    fn should_send(&mut self, pkt: &mut MediaPacket) -> (FilterResult, bool);
}

enum CodecFilter {
    Vp8(vp8_sim::Vp8SimulcastFilter),
    Vp9(vp9_svc::Vp9SvcFilter),
    H264(h264_sim::H264SimulcastFilter),
    Video(video_single::VideoSingleFilter),
    Passthrough,
}

impl CodecFilter {
    pub fn should_send(&mut self, pkt: &mut MediaPacket) -> (FilterResult, bool) {
        match self {
            CodecFilter::Vp8(filter) => filter.should_send(pkt),
            CodecFilter::Vp9(filter) => filter.should_send(pkt),
            CodecFilter::H264(filter) => filter.should_send(pkt),
            CodecFilter::Video(filter) => filter.should_send(pkt),
            CodecFilter::Passthrough => (FilterResult::Send, false),
        }
    }

    pub fn pause(&mut self) {
        match self {
            CodecFilter::Vp8(filter) => filter.pause(),
            CodecFilter::Vp9(filter) => filter.pause(),
            CodecFilter::H264(filter) => filter.pause(),
            CodecFilter::Video(filter) => filter.pause(),
            CodecFilter::Passthrough => {}
        }
    }

    pub fn resume(&mut self) {
        match self {
            CodecFilter::Vp8(filter) => filter.resume(),
            CodecFilter::Vp9(filter) => filter.resume(),
            CodecFilter::H264(filter) => filter.resume(),
            CodecFilter::Video(filter) => filter.resume(),
            CodecFilter::Passthrough => {}
        }
    }

    pub fn set_target(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool {
        match self {
            CodecFilter::Vp8(filter) => filter.set_target_layer(spatial, temporal, key_only),
            CodecFilter::Vp9(filter) => filter.set_target_layer(spatial, temporal, key_only),
            CodecFilter::H264(filter) => filter.set_target_layer(spatial, temporal, key_only),
            CodecFilter::Video(filter) => filter.set_target_layer(spatial, temporal, key_only),
            CodecFilter::Passthrough => false,
        }
    }
}

pub struct ScalablePacketFilter {
    pause: bool,
    previous_target: Option<(u8, u8, bool)>,
    filter: Option<CodecFilter>,
    seq_rewrite: MediaSeqRewrite,
    ts_rewrite: MediaTsRewrite,
}

impl ScalablePacketFilter {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            pause: false,
            previous_target: None,
            filter: None,
            seq_rewrite: MediaSeqRewrite::default(),
            ts_rewrite: MediaTsRewrite::new(sample_rate as u64),
        }
    }

    pub fn switched_source(&mut self) {
        self.filter = None;
        self.previous_target = None;
        self.ts_rewrite.reinit();
        self.seq_rewrite.reinit();
    }

    /// set target and return true if need a key frame
    pub fn set_target(&mut self, target: LocalTrackTarget) -> bool {
        match target {
            LocalTrackTarget::WaitStart => {
                self.filter = None;
                true
            }
            LocalTrackTarget::Pause => {
                self.pause = true;
                if let Some(filter) = &mut self.filter {
                    filter.pause();
                }
                self.ts_rewrite.reinit();
                self.seq_rewrite.reinit();
                false
            }
            LocalTrackTarget::Single { key_only } => {
                if self.pause {
                    self.pause = false;
                    if let Some(filter) = &mut self.filter {
                        filter.resume();
                    }
                }

                self.previous_target = Some((2, 2, key_only));
                if let Some(filter) = &mut self.filter {
                    filter.set_target(2, 2, key_only)
                } else {
                    false
                }
            }
            LocalTrackTarget::Scalable { spatial, temporal, key_only } => {
                if self.pause {
                    self.pause = false;
                    if let Some(filter) = &mut self.filter {
                        filter.resume();
                    }
                }

                self.previous_target = Some((spatial, temporal, key_only));
                if let Some(filter) = &mut self.filter {
                    filter.set_target(spatial, temporal, key_only)
                } else {
                    false
                }
            }
        }
    }

    pub fn process(&mut self, now_ms: u64, mut pkt: MediaPacket) -> Option<MediaPacket> {
        if self.pause {
            return None;
        }

        let pre_codec = pkt.codec.clone();

        let (filter_res, reinit) = match &mut self.filter {
            Some(filter) => filter.should_send(&mut pkt),
            None => {
                let mut filter = match &pkt.codec {
                    PayloadCodec::Vp8(_, Some(_)) => CodecFilter::Vp8(Vp8SimulcastFilter::default()),
                    PayloadCodec::Vp9(_, _, Some(_)) => CodecFilter::Vp9(Vp9SvcFilter::new(false)),
                    PayloadCodec::H264(_, _, Some(_)) => CodecFilter::H264(H264SimulcastFilter::default()),
                    PayloadCodec::Vp8(_, None) | PayloadCodec::Vp9(_, _, None) | PayloadCodec::H264(_, _, None) => CodecFilter::Video(VideoSingleFilter::default()),
                    _ => CodecFilter::Passthrough,
                };

                if let Some((spatial, temporal, key_only)) = &self.previous_target {
                    filter.set_target(*spatial, *temporal, *key_only);
                }

                let res = filter.should_send(&mut pkt);
                self.filter = Some(filter);
                res
            }
        };

        if reinit {
            self.ts_rewrite.reinit();
            self.seq_rewrite.reinit();
        }

        match filter_res {
            FilterResult::Send => {
                let seq = self.seq_rewrite.generate(pkt.seq_no as u64)?;
                let ts = self.ts_rewrite.generate(now_ms, pkt.time as u64);
                log::debug!("[ScalablePacketFilter] rewrite {} {} {} => to {}, {}, {}", pkt.seq_no, pkt.time, pre_codec, seq, ts, pkt.codec);
                pkt.time = ts as u32;
                pkt.seq_no = seq as u16;
                Some(pkt)
            }
            FilterResult::Drop => {
                log::debug!("[ScalablePacketFilter] drop {} {} {}", pkt.seq_no, pkt.time, pre_codec);
                self.seq_rewrite.drop_value(pkt.seq_no as u64);
                None
            }
            FilterResult::Reject => {
                log::debug!("[ScalablePacketFilter] reject {} {} {}", pkt.seq_no, pkt.time, pre_codec);
                None
            }
        }
    }
}

#[cfg(test)]
mod test {}
