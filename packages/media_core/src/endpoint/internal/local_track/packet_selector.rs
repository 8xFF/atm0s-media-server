use std::collections::VecDeque;

use media_server_protocol::media::{MediaKind, MediaMeta, MediaPacket};
use media_server_utils::{SeqRewrite, TsRewrite};

mod video_h264_sim;
mod video_single;
mod video_vp8_sim;
mod video_vp9_svc;

const REQUEST_KEY_FRAME_INTERVAL_MS: u64 = 100; //only allow request keyframe each 100ms
const SEQ_MAX: u64 = 1 << 16;
const TS_MAX: u64 = 1 << 32;

pub type MediaSeqRewrite = SeqRewrite<SEQ_MAX, 1000>;
pub type MediaTsRewrite = TsRewrite<TS_MAX, 10>;

trait VideoSelector {
    fn on_tick(&mut self, now_ms: u64);
    fn set_target_bitrate(&mut self, now_ms: u64, bitrate: u64);
    fn selector(&mut self, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()>;
    fn pop_action(&mut self) -> Option<Action>;
}

pub enum Action {
    RequestKeyFrame,
    DesiredBitrate(u64),
}

pub struct PacketSelector {
    kind: MediaKind,
    ts_rewrite: MediaTsRewrite,
    seq_rewrite: MediaSeqRewrite,
    selected_channel: Option<u64>,
    need_key_frame: bool,
    last_key_frame_ts: Option<u64>,
    selector: Option<Box<dyn VideoSelector>>,
    queue: VecDeque<Action>,
}

impl PacketSelector {
    pub fn new(kind: MediaKind) -> Self {
        Self {
            kind,
            ts_rewrite: MediaTsRewrite::new(kind.sample_rate()),
            seq_rewrite: MediaSeqRewrite::default(),
            selected_channel: None,
            need_key_frame: false,
            last_key_frame_ts: None,
            selector: None,
            queue: VecDeque::new(),
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        self.selector.as_mut().map(|s| s.on_tick(now_ms));
        if self.need_key_frame {
            if self.last_key_frame_ts.is_none() || self.last_key_frame_ts.expect("Should have") + REQUEST_KEY_FRAME_INTERVAL_MS <= now_ms {
                self.last_key_frame_ts = Some(now_ms);
                self.queue.push_back(Action::RequestKeyFrame);
            }
        }
    }

    /// Reset, call reset if local_track changed source
    pub fn reset(&mut self) {
        log::info!("[LocalTrack/PacketSelector] reset");
        self.selected_channel = None;
        self.selector = None;
        self.need_key_frame = false;
        self.last_key_frame_ts = None;
    }

    /// Set target bitrate, which is used to select best layer for avoiding freezes or lags
    pub fn set_target_bitrate(&mut self, now_ms: u64, bitrate: u64) {
        self.selector.as_mut().map(|s| s.set_target_bitrate(now_ms, bitrate));
    }

    /// Select and rewrite if need. If select will return Some<()>
    pub fn select(&mut self, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()> {
        if self.selected_channel != Some(channel) {
            log::info!("[LocalTrack/PacketSelector] source changed => reinit ts_rewrite and seq_rewrite");
            self.ts_rewrite.reinit();
            self.seq_rewrite.reinit();
            self.selected_channel = Some(channel);
            self.selector = match pkt.meta {
                MediaMeta::Opus { .. } => None,
                MediaMeta::H264 { sim: Some(sim), .. } => todo!(),
                MediaMeta::Vp8 { sim: Some(sim), .. } => todo!(),
                MediaMeta::Vp9 { svc: Some(svc), .. } => todo!(),
                MediaMeta::H264 { sim: None, .. } | MediaMeta::Vp8 { sim: None, .. } | MediaMeta::Vp9 { svc: None, .. } => {
                    log::info!("[LocalTrack/PacketSelector] create VideoSingleSelector");
                    Some(Box::new(video_single::VideoSingleSelector::default()))
                }
            };

            if self.kind.is_video() {
                //only video type is need key-frame
                self.queue.push_back(Action::RequestKeyFrame);
                self.need_key_frame = true;
                self.last_key_frame_ts = Some(now_ms);
            }
        }

        self.selector.as_mut().map(|s| s.selector(now_ms, channel, pkt)).unwrap_or(Some(()))?;

        pkt.ts = self.ts_rewrite.generate(now_ms, pkt.ts as u64) as u32;
        pkt.seq = self.seq_rewrite.generate(pkt.seq as u64)? as u16;

        if self.need_key_frame && pkt.meta.is_video_key() {
            self.need_key_frame = false;
        }

        Some(())
    }

    pub fn pop_output(&mut self, now_ms: u64) -> Option<Action> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        while let Some(out) = self.selector.as_mut()?.pop_action() {
            match out {
                Action::RequestKeyFrame => {
                    if self.last_key_frame_ts.is_none() || self.last_key_frame_ts.expect("Should have") + REQUEST_KEY_FRAME_INTERVAL_MS <= now_ms {
                        self.last_key_frame_ts = Some(now_ms);
                        return Some(Action::RequestKeyFrame);
                    }
                }
                Action::DesiredBitrate(bitrate) => return Some(Action::DesiredBitrate(bitrate)),
            }
        }

        None
    }
}
