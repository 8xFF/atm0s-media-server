//! Single stream video selector.
//! This selector allow all video because parent PacketSelector already wait for key-frame

use super::{VideoSelector, VideoSelectorCtx};

#[derive(Default)]
pub struct VideoSingleSelector {}

impl VideoSelector for VideoSingleSelector {
    fn on_init(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64) {}

    fn on_tick(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64) {}

    fn set_target_bitrate(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64, _bitrate: u64) {}

    fn set_limit_layer(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64, _max_spatial: u8, _max_temporal: u8) {}

    fn select(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64, _channel: u64, _pkt: &mut media_server_protocol::media::MediaPacket) -> Option<()> {
        Some(())
    }

    fn pop_action(&mut self) -> Option<super::Action> {
        None
    }
}
