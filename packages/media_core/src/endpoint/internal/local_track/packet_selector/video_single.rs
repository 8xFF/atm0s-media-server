use std::collections::VecDeque;

use super::{Action, VideoSelector};

#[derive(Default)]
pub struct VideoSingleSelector {
    has_key: bool,
    queue: VecDeque<Action>,
}

impl VideoSelector for VideoSingleSelector {
    fn on_tick(&mut self, _now_ms: u64) {}

    fn set_target_bitrate(&mut self, _now_ms: u64, bitrate: u64) {
        self.queue.push_back(Action::DesiredBitrate(bitrate))
    }

    fn selector(&mut self, _now_ms: u64, _channel: u64, pkt: &mut media_server_protocol::media::MediaPacket) -> Option<()> {
        if !self.has_key && pkt.meta.is_video_key() {
            log::info!("[VideoSingleSelector] first key-frame {} arrived => switch to live mode", pkt.seq);
            self.has_key = true;
        }

        if self.has_key {
            Some(())
        } else {
            log::debug!("[VideoSingleSelector] wait first key-frame => reject {}", pkt.seq);
            None
        }
    }

    fn pop_action(&mut self) -> Option<super::Action> {
        self.queue.pop_front()
    }
}
