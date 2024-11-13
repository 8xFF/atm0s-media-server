//! PacketSelector take care core SFU part
//! It will determine which packet is allow, and which will be dropped
//!
//! Main job:
//!
//! - Request key-frame at first
//! - Create selector based on request

use std::collections::VecDeque;

use media_server_protocol::media::{MediaKind, MediaLayersBitrate, MediaMeta, MediaPacket};
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

/// Implement selector logic
/// Note that, inside selector logic must to implement SeqRewrite drop_value
/// or SeqRewrite and TsRewrite reset if needed (seems Simulcast will need to do that)
trait VideoSelector {
    fn on_init(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64);
    fn on_tick(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64);
    fn set_target_bitrate(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64, bitrate: u64);
    fn set_limit_layer(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64, max_spatial: u8, max_temporal: u8);
    fn select(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<()>;
    fn pop_action(&mut self) -> Option<Action>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    RequestKeyFrame,
}

pub struct VideoSelectorCtx {
    pub ts_rewrite: MediaTsRewrite,
    pub seq_rewrite: MediaSeqRewrite,
    //TODO beterway to store codec specific state
    pub vp8_ctx: video_vp8_sim::Ctx,
    //TODO beterway to store codec specific state
    pub vp9_ctx: video_vp9_svc::Ctx,
}

impl VideoSelectorCtx {
    pub fn new(kind: MediaKind) -> Self {
        Self {
            ts_rewrite: MediaTsRewrite::new(kind.sample_rate()),
            seq_rewrite: MediaSeqRewrite::default(),
            vp8_ctx: Default::default(),
            vp9_ctx: Default::default(),
        }
    }
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
    limit: (u8, u8),
}

impl PacketSelector {
    pub fn new(kind: MediaKind, max_spatial: u8, max_temporal: u8) -> Self {
        Self {
            kind,
            ctx: VideoSelectorCtx::new(kind),
            selected_channel: None,
            need_key_frame: false,
            last_key_frame_ts: None,
            selector: None,
            queue: VecDeque::new(),
            bitrate: None,
            limit: (max_spatial, max_temporal),
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        if let Some(s) = self.selector.as_mut() {
            s.on_tick(&mut self.ctx, now_ms);
        }
        if self.need_key_frame && (self.last_key_frame_ts.is_none() || self.last_key_frame_ts.expect("Should have") + REQUEST_KEY_FRAME_INTERVAL_MS <= now_ms) {
            log::info!("[LocalTrack/PacketSelector] on_tick => request key after interval");
            self.last_key_frame_ts = Some(now_ms);
            self.queue.push_back(Action::RequestKeyFrame);
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
        log::debug!("[LocalTrack/PacketSelector] set target bitrate to {}", bitrate);
        self.bitrate = Some(bitrate);
        if let Some(s) = self.selector.as_mut() {
            s.set_target_bitrate(&mut self.ctx, now_ms, bitrate);
        }
    }

    /// Set limit layer, which is used for select best layer
    pub fn set_limit_layer(&mut self, now_ms: u64, max_spatial: u8, min_spatial: u8) {
        self.limit = (max_spatial, min_spatial);
        if let Some(s) = self.selector.as_mut() {
            s.set_limit_layer(&mut self.ctx, now_ms, max_spatial, min_spatial);
        }
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
            } else {
                log::info!("[LocalTrack/PacketSelector] video source changed and first pkt is key");
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
            self.selector = create_selector(pkt, bitrate, self.limit);
            self.selector.as_mut().expect("Should have video selector").on_init(&mut self.ctx, now_ms);
        }

        self.selector.as_mut()?.select(&mut self.ctx, now_ms, channel, pkt)
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

fn create_selector(pkt: &MediaPacket, bitrate: u64, limit: (u8, u8)) -> Option<Box<dyn VideoSelector>> {
    match &pkt.meta {
        MediaMeta::Opus { .. } => {
            log::info!("[LocalTrack/PacketSelector] dont create Selector for audio");
            None
        }
        MediaMeta::H264 { sim: Some(_), .. } => {
            let layers = pkt.layers.clone().unwrap_or_else(MediaLayersBitrate::default_sim);
            log::info!("[LocalTrack/PacketSelector] create H264SimSelector");
            Some(Box::new(video_h264_sim::Selector::new(bitrate, layers.clone(), limit)))
        }
        MediaMeta::Vp8 { sim: Some(_), .. } => {
            let layers = pkt.layers.clone().unwrap_or_else(MediaLayersBitrate::default_sim);
            log::info!("[LocalTrack/PacketSelector] create Vp8SimSelector");
            Some(Box::new(video_vp8_sim::Selector::new(bitrate, layers.clone(), limit)))
        }
        MediaMeta::Vp9 { svc: Some(_), .. } => {
            let layers = pkt.layers.clone().unwrap_or_else(MediaLayersBitrate::default_sim);
            log::info!("[LocalTrack/PacketSelector] create Vp9SvcSelector");
            Some(Box::new(video_vp9_svc::Selector::new(false, bitrate, layers.clone(), limit)))
        }
        MediaMeta::H264 { sim: None, .. } | MediaMeta::Vp8 { sim: None, .. } | MediaMeta::Vp9 { svc: None, .. } => {
            log::info!("[LocalTrack/PacketSelector] create VideoSingleSelector");
            Some(Box::<video_single::VideoSingleSelector>::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use media_server_protocol::media::{MediaKind, MediaMeta, MediaPacket};

    use super::{Action, PacketSelector, REQUEST_KEY_FRAME_INTERVAL_MS};

    fn audio_pkt() -> MediaPacket {
        MediaPacket {
            ts: 0,
            seq: 0,
            marker: true,
            nackable: false,
            layers: None,
            meta: MediaMeta::Opus { audio_level: None },
            data: vec![1, 2, 3],
        }
    }

    fn video_pkt(key: bool) -> MediaPacket {
        MediaPacket {
            ts: 0,
            seq: 0,
            marker: true,
            nackable: false,
            layers: None,
            meta: MediaMeta::Vp8 { key, sim: None, rotation: None },
            data: vec![1, 2, 3],
        }
    }

    #[test_log::test]
    fn audio_should_not_request_key_frame() {
        let mut selector = PacketSelector::new(MediaKind::Audio, 2, 2);

        let mut pkt = audio_pkt();
        assert_eq!(selector.select(0, 0, &mut pkt), Some(()));
        assert_eq!(selector.pop_output(0), None);
    }

    #[test_log::test]
    fn video_should_not_request_key_frame_with_first_is_key() {
        let mut selector = PacketSelector::new(MediaKind::Video, 2, 2);

        selector.set_target_bitrate(0, 2_000_000);

        let mut pkt = video_pkt(true);
        assert_eq!(selector.select(0, 0, &mut pkt), Some(()));
        assert_eq!(selector.pop_output(0), None);
    }

    #[test_log::test]
    fn video_should_request_key_frame_with_first_is_not_key() {
        let mut selector = PacketSelector::new(MediaKind::Video, 2, 2);

        selector.set_target_bitrate(0, 2_000_000);

        let mut pkt = video_pkt(false);
        assert_eq!(selector.select(0, 0, &mut pkt), None);
        assert_eq!(selector.pop_output(0), Some(Action::RequestKeyFrame));
        assert_eq!(selector.pop_output(0), None);

        selector.on_tick(1);
        assert_eq!(selector.pop_output(0), None);

        //will retry after interval
        selector.on_tick(REQUEST_KEY_FRAME_INTERVAL_MS);
        assert_eq!(selector.pop_output(0), Some(Action::RequestKeyFrame));
        assert_eq!(selector.pop_output(0), None);

        //after receive key-frame will stop request key-frame
        let mut pkt2 = video_pkt(true);
        assert_eq!(selector.select(0, 0, &mut pkt2), Some(()));
        assert_eq!(selector.pop_output(0), None);

        selector.on_tick(2 * REQUEST_KEY_FRAME_INTERVAL_MS);
        assert_eq!(selector.pop_output(0), None);
    }

    #[test_log::test]
    fn pkt_rewrite_after_switch_channel() {}
}
