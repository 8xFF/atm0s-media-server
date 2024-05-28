use std::collections::VecDeque;

use media_server_protocol::endpoint::TrackPriority;

use crate::transport::LocalTrackId;

const DEFAULT_BITRATE_BPS: u64 = 800_000;
const NO_TRACK_BWE_CURRENT: u64 = 100_000;
const NO_TRACK_BWE_DESIRED: u64 = 300_000;

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    SetBitrate(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output {
    Track(LocalTrackId, Action),
    BweConfig(u64, u64),
}

pub struct EgressBitrateAllocator {
    max_egress_bitrate: u64,
    changed: bool,
    egress_bitrate: u64,
    tracks: smallmap::Map<LocalTrackId, TrackPriority>,
    queue: VecDeque<Output>,
}

impl EgressBitrateAllocator {
    pub fn new(max_egress_bitrate: u64) -> Self {
        Self {
            max_egress_bitrate,
            changed: false,
            egress_bitrate: DEFAULT_BITRATE_BPS,
            tracks: smallmap::Map::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_tick(&mut self) {
        self.process();
    }

    pub fn set_egress_estimate(&mut self, bitrate: u64) {
        self.egress_bitrate = bitrate;
        self.changed = true;
    }

    pub fn set_video_track(&mut self, track: LocalTrackId, priority: TrackPriority) {
        log::info!("[EgressBitrateAllocator] set video track {track} priority {priority}");
        self.tracks.insert(track, priority);
        self.changed = true;
    }

    pub fn del_video_track(&mut self, track: LocalTrackId) {
        log::info!("[EgressBitrateAllocator] del video track {track}");
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
        let use_bitrate = self.egress_bitrate.min(self.max_egress_bitrate);
        let mut sum = TrackPriority(0);
        for (_track, priority) in self.tracks.iter() {
            sum += *priority;
        }

        if *(sum.as_ref()) != 0 {
            for (track, priority) in self.tracks.iter() {
                let bitrate = (use_bitrate * priority.0 as u64) / sum.0 as u64;
                log::debug!("[EgressBitrateAllocator] set track {track} with bitrate {bitrate}");
                self.queue.push_back(Output::Track(*track, Action::SetBitrate(bitrate)));
            }
        }

        if !self.tracks.is_empty() {
            //TODO fix issue when config max_egress_bitrate is lower than stream bitrate, this will make BWE pacer
            //slow down sending packet, then latency of viewer will be increase
            let current = use_bitrate;
            let desired = (use_bitrate * 6 / 5).min(self.max_egress_bitrate);
            log::debug!("[EgressBitrateAllocator] set bwe config current {current}, desired {desired}");
            self.queue.push_back(Output::BweConfig(current, desired));
        } else {
            log::debug!(
                "[EgressBitrateAllocator] set bwe config without tracks => current {}, desired {}",
                NO_TRACK_BWE_CURRENT,
                NO_TRACK_BWE_DESIRED
            );
            self.queue.push_back(Output::BweConfig(NO_TRACK_BWE_CURRENT, NO_TRACK_BWE_DESIRED));
        }
    }
}

#[cfg(test)]
mod test {
    use crate::endpoint::internal::bitrate_allocator::egress::{EgressBitrateAllocator, NO_TRACK_BWE_CURRENT, NO_TRACK_BWE_DESIRED};

    use super::{Action, Output, DEFAULT_BITRATE_BPS};

    const MAX_BW: u64 = 2_500_000;

    #[test]
    fn no_source() {
        let mut allocator = EgressBitrateAllocator::new(MAX_BW);
        allocator.set_egress_estimate(200_000);
        allocator.on_tick();

        assert_eq!(allocator.pop_output(), Some(Output::BweConfig(NO_TRACK_BWE_CURRENT, NO_TRACK_BWE_DESIRED)));
        assert_eq!(allocator.pop_output(), None);
    }

    #[test]
    fn single_source() {
        let mut allocator = EgressBitrateAllocator::new(MAX_BW);
        allocator.set_video_track(0.into(), 1.into());

        allocator.on_tick();
        assert_eq!(allocator.pop_output(), Some(Output::Track(0.into(), Action::SetBitrate(DEFAULT_BITRATE_BPS))));
        assert_eq!(allocator.pop_output(), Some(Output::BweConfig(DEFAULT_BITRATE_BPS, DEFAULT_BITRATE_BPS * 6 / 5)));
        assert_eq!(allocator.pop_output(), None);

        //test with estimate bitrate over MAX_BITRATE should cap
        allocator.set_egress_estimate(MAX_BW + 200_000);
        allocator.on_tick();
        assert_eq!(allocator.pop_output(), Some(Output::Track(0.into(), Action::SetBitrate(MAX_BW))));
        assert_eq!(allocator.pop_output(), Some(Output::BweConfig(MAX_BW, MAX_BW)));
        assert_eq!(allocator.pop_output(), None);
    }

    #[test]
    fn multi_source() {
        let mut allocator = EgressBitrateAllocator::new(MAX_BW);
        allocator.set_video_track(0.into(), 1.into());
        allocator.set_video_track(1.into(), 3.into());

        allocator.on_tick();
        assert_eq!(allocator.pop_output(), Some(Output::Track(0.into(), Action::SetBitrate(DEFAULT_BITRATE_BPS * 1 / 4))));
        assert_eq!(allocator.pop_output(), Some(Output::Track(1.into(), Action::SetBitrate(DEFAULT_BITRATE_BPS * 3 / 4))));
        assert_eq!(allocator.pop_output(), Some(Output::BweConfig(DEFAULT_BITRATE_BPS, DEFAULT_BITRATE_BPS * 6 / 5)));
        assert_eq!(allocator.pop_output(), None);
    }
}
