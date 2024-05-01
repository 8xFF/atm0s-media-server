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
    fn on_init(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64);
    fn on_tick(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64);
    fn set_target_bitrate(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64, bitrate: u64);
    fn selector(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()>;
    fn pop_action(&mut self) -> Option<Action>;
}

pub enum Action {
    RequestKeyFrame,
}

pub struct VideoSelectorCtx {
    pub ts_rewrite: MediaTsRewrite,
    pub seq_rewrite: MediaSeqRewrite,
    pub vp8_ctx: video_vp8_sim::Ctx,
}

pub struct PacketSelector {
    kind: MediaKind,
    ctx: VideoSelectorCtx,
    selected_channel: Option<u64>,
    need_key_frame: bool,
    last_key_frame_ts: Option<u64>,
    selector: Option<Box<dyn VideoSelector>>,
    queue: VecDeque<Action>,
    bitrate: Option<u64>,
}

impl PacketSelector {
    pub fn new(kind: MediaKind) -> Self {
        Self {
            kind,
            ctx: VideoSelectorCtx {
                ts_rewrite: MediaTsRewrite::new(kind.sample_rate()),
                seq_rewrite: MediaSeqRewrite::default(),
                vp8_ctx: Default::default(),
            },
            selected_channel: None,
            need_key_frame: false,
            last_key_frame_ts: None,
            selector: None,
            queue: VecDeque::new(),
            bitrate: None,
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        self.selector.as_mut().map(|s| s.on_tick(&mut self.ctx, now_ms));
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
        self.need_key_frame = false;
        self.last_key_frame_ts = None;
        self.bitrate = None;
    }

    /// Set target bitrate, which is used to select best layer for avoiding freezes or lags
    pub fn set_target_bitrate(&mut self, now_ms: u64, bitrate: u64) {
        log::info!("[LocalTrack/PacketSelector] set target bitrate to {}", bitrate);
        self.bitrate = Some(bitrate);
        self.selector.as_mut().map(|s| s.set_target_bitrate(&mut self.ctx, now_ms, bitrate));
    }

    pub fn select(&mut self, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()> {
        if self.kind.is_audio() {
            self.select_audio(now_ms, channel, pkt)
        } else {
            self.select_video(now_ms, channel, pkt)
        }
    }

    /// Select audio is simple allow all
    fn select_audio(&mut self, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()> {
        if self.selected_channel != Some(channel) {
            log::info!("[LocalTrack/PacketSelector] audio source changed => reinit ts_rewrite, seq_rewrite and clear selector");
            self.ctx.ts_rewrite.reinit();
            self.ctx.seq_rewrite.reinit();
            self.selected_channel = Some(channel);
        }

        pkt.ts = self.ctx.ts_rewrite.generate(now_ms, pkt.ts as u64) as u32;
        pkt.seq = self.ctx.seq_rewrite.generate(pkt.seq as u64)? as u16;

        Some(())
    }

    /// Select and rewrite if need. If select will return Some<()>
    fn select_video(&mut self, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()> {
        if self.select_video_inner(now_ms, channel, pkt).is_some() {
            //allow
            log::trace!("[LocalTrack/PacketSelector] video allow {} {}", pkt.seq, pkt.ts);
            pkt.ts = self.ctx.ts_rewrite.generate(now_ms, pkt.ts as u64) as u32;
            pkt.seq = self.ctx.seq_rewrite.generate(pkt.seq as u64)? as u16;
            Some(())
        } else {
            //drop
            log::trace!("[LocalTrack/PacketSelector] video reject {} {}", pkt.seq, pkt.ts);
            None
        }
    }

    fn select_video_inner(&mut self, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()> {
        if self.selected_channel != Some(channel) {
            log::info!("[LocalTrack/PacketSelector] video source changed => reinit ts_rewrite, seq_rewrite and clear selector");
            self.ctx.ts_rewrite.reinit();
            self.ctx.seq_rewrite.reinit();
            self.selected_channel = Some(channel);
            self.selector = None;

            //if first pkt is not key, we need request it
            if !pkt.meta.is_video_key() {
                log::info!("[LocalTrack/PacketSelector] video source changed but first pkt isn't key => request key frame");
                self.queue.push_back(Action::RequestKeyFrame);
                self.need_key_frame = true;
                self.last_key_frame_ts = Some(now_ms);
            }
        }

        let bitrate = self.bitrate?;
        if self.need_key_frame && pkt.meta.is_video_key() {
            log::info!(
                "[LocalTrack/PacketSelector] found key frame {:?}, source layers {:?}, target bitrate {:?}",
                pkt.meta,
                pkt.layers,
                self.bitrate
            );
            self.need_key_frame = false;
        }
        if self.selector.is_none() && pkt.meta.is_video_key() {
            self.selector = match pkt.meta {
                MediaMeta::Opus { .. } => None,
                MediaMeta::H264 { sim: Some(_), .. } => todo!(),
                MediaMeta::Vp8 { sim: Some(_), .. } => {
                    let layers = pkt.layers.as_ref()?;
                    log::info!("[LocalTrack/PacketSelector] create Vp8SimSelector");
                    Some(Box::new(video_vp8_sim::Selector::new(bitrate, layers.clone())))
                }
                MediaMeta::Vp9 { svc: Some(_), .. } => todo!(),
                MediaMeta::H264 { sim: None, .. } | MediaMeta::Vp8 { sim: None, .. } | MediaMeta::Vp9 { svc: None, .. } => {
                    log::info!("[LocalTrack/PacketSelector] create VideoSingleSelector");
                    Some(Box::new(video_single::VideoSingleSelector::default()))
                }
            };

            self.selector.as_mut().map(|s| s.on_init(&mut self.ctx, now_ms));
        }

        self.selector.as_mut()?.selector(&mut self.ctx, now_ms, channel, pkt)
    }

    pub fn pop_output(&mut self, now_ms: u64) -> Option<Action> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        while let Some(out) = self.selector.as_mut()?.pop_action() {
            match out {
                Action::RequestKeyFrame => {
                    if self.last_key_frame_ts.is_none() || self.last_key_frame_ts.expect("Should have") + REQUEST_KEY_FRAME_INTERVAL_MS <= now_ms {
                        self.need_key_frame = true;
                        self.last_key_frame_ts = Some(now_ms);
                        return Some(Action::RequestKeyFrame);
                    }
                }
            }
        }

        None
    }
}
