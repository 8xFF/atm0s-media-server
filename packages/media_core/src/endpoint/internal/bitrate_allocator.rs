use derivative::Derivative;
use std::collections::VecDeque;

use media_server_protocol::endpoint::TrackPriority;

use crate::transport::LocalTrackId;

const DEFAULT_BITRATE_BPS: u64 = 800_000;

#[derive(Debug, PartialEq, Eq)]
pub enum Output {
    SetTrackBitrate(LocalTrackId, u64),
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct BitrateAllocator {
    changed: bool,
    #[derivative(Default(value = "DEFAULT_BITRATE_BPS"))]
    egress_bitrate: u64,
    tracks: smallmap::Map<LocalTrackId, TrackPriority>,
    queue: VecDeque<Output>,
}

impl BitrateAllocator {
    pub fn on_tick(&mut self) {
        self.process();
    }

    pub fn set_egress_bitrate(&mut self, bitrate: u64) {
        self.egress_bitrate = bitrate;
        self.changed = true;
    }

    pub fn set_video_track(&mut self, track: LocalTrackId, priority: TrackPriority) {
        self.tracks.insert(track, priority);
        self.changed = true;
    }

    pub fn del_video_track(&mut self, track: LocalTrackId) {
        self.tracks.remove(&track);
        self.changed = true;
    }

    pub fn pop_output(&mut self) -> Option<Output> {
        self.queue.pop_front()
    }

    fn process(&mut self) {
        if !self.changed {
            return;
        }
        self.changed = false;
        let mut sum = TrackPriority(0);
        for (_track, priority) in self.tracks.iter() {
            sum = sum + *priority;
        }

        if *(sum.as_ref()) != 0 {
            for (track, priority) in self.tracks.iter() {
                self.queue.push_back(Output::SetTrackBitrate(*track, (self.egress_bitrate * priority.0 as u64) / sum.0 as u64));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::{BitrateAllocator, Output, DEFAULT_BITRATE_BPS};

    #[test]
    fn single_source() {
        let mut allocator = BitrateAllocator::default();
        allocator.set_video_track(0.into(), 1.into());

        allocator.on_tick();
        assert_eq!(allocator.pop_output(), Some(Output::SetTrackBitrate(0.into(), DEFAULT_BITRATE_BPS)));
    }

    #[test]
    fn multi_source() {
        let mut allocator = BitrateAllocator::default();
        allocator.set_video_track(0.into(), 1.into());
        allocator.set_video_track(1.into(), 3.into());

        allocator.on_tick();
        assert_eq!(allocator.pop_output(), Some(Output::SetTrackBitrate(0.into(), DEFAULT_BITRATE_BPS * 1 / 4)));
        assert_eq!(allocator.pop_output(), Some(Output::SetTrackBitrate(1.into(), DEFAULT_BITRATE_BPS * 3 / 4)));
    }
}
