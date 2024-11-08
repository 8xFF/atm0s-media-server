use std::collections::VecDeque;

use media_server_protocol::endpoint::TrackPriority;

use crate::transport::RemoteTrackId;

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    SetBitrate(u64),
}

pub struct IngressBitrateAllocator {
    changed: bool,
    ingress_bitrate: u64,
    tracks: smallmap::Map<RemoteTrackId, TrackPriority>,
    queue: VecDeque<(RemoteTrackId, Action)>,
}

impl IngressBitrateAllocator {
    pub fn new(ingress_bitrate: u64) -> Self {
        Self {
            ingress_bitrate,
            changed: false,
            tracks: smallmap::Map::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_tick(&mut self) {
        self.process();
    }

    pub fn set_video_track(&mut self, track: RemoteTrackId, priority: TrackPriority) {
        log::info!("[IngressBitrateAllocator] set video track {track} priority {priority}");
        self.tracks.insert(track, priority);
        self.changed = true;
    }

    pub fn del_video_track(&mut self, track: RemoteTrackId) {
        log::info!("[IngressBitrateAllocator] del video track {track}");
        self.tracks.remove(&track);
        self.changed = true;
    }

    pub fn pop_output(&mut self) -> Option<(RemoteTrackId, Action)> {
        self.queue.pop_front()
    }

    fn process(&mut self) {
        if !self.changed {
            return;
        }
        self.changed = false;
        let mut sum = TrackPriority::from(0);
        for (_track, priority) in self.tracks.iter() {
            sum += *priority;
        }

        if *(sum.as_ref()) != 0 {
            for (track, priority) in self.tracks.iter() {
                let bitrate = (self.ingress_bitrate * (**priority) as u64) / *sum as u64;
                log::debug!("[IngressBitrateAllocator] set track {track} with bitrate {bitrate}");
                self.queue.push_back((*track, Action::SetBitrate(bitrate)));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::endpoint::internal::bitrate_allocator::ingress::IngressBitrateAllocator;

    use super::Action;

    const TEST_BITRATE: u64 = 2_000_000;

    #[test_log::test]
    fn single_source() {
        let mut allocator = IngressBitrateAllocator::new(TEST_BITRATE);
        allocator.set_video_track(0.into(), 1.into());

        allocator.on_tick();
        assert_eq!(allocator.pop_output(), Some((0.into(), Action::SetBitrate(TEST_BITRATE))));
    }

    #[test_log::test]
    fn multi_source() {
        let mut allocator = IngressBitrateAllocator::new(TEST_BITRATE);
        allocator.set_video_track(0.into(), 1.into());
        allocator.set_video_track(1.into(), 3.into());

        allocator.on_tick();
        assert_eq!(allocator.pop_output(), Some((0.into(), Action::SetBitrate(TEST_BITRATE / 4))));
        assert_eq!(allocator.pop_output(), Some((1.into(), Action::SetBitrate(TEST_BITRATE * 3 / 4))));
    }
}
