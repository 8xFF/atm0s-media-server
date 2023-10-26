use std::collections::{HashMap, VecDeque};

use cluster::ClusterTrackStats;
use transport::TrackId;

use crate::rpc::ReceiverLayerLimit;

const SINGLE_STREAM_BASED_BITRATE: u32 = 80_000; //100kbps
const SIMULCAST_BASED_BITRATE: u32 = 60_000; //60kbps
const SVC_BASED_BITRATE: u32 = 60_000; //60kbps

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalTrackTarget {
    WaitStart,
    Pause,
    Single { key_only: bool },
    Scalable { spatial: u8, temporal: u8, key_only: bool },
}

#[derive(Debug, PartialEq, Eq)]
pub enum BitrateAllocationAction {
    LimitLocalTrack(TrackId, LocalTrackTarget),
    LimitLocalTrackBitrate(TrackId, u32),
}

struct TrackSlot {
    track_id: u16,
    limit: ReceiverLayerLimit,
    target: Option<LocalTrackTarget>,
    source: Option<ClusterTrackStats>,
}

impl TrackSlot {
    pub fn priority(&self) -> u16 {
        self.limit.priority
    }

    pub fn based_bitrate(&self) -> u32 {
        match &self.source {
            Some(source) => match source {
                ClusterTrackStats::Single { bitrate: _ } => SINGLE_STREAM_BASED_BITRATE,
                ClusterTrackStats::Simulcast { bitrate: _, layers: _ } => SIMULCAST_BASED_BITRATE,
                ClusterTrackStats::Svc { bitrate: _, layers: _ } => SVC_BASED_BITRATE,
            },
            None => SINGLE_STREAM_BASED_BITRATE,
        }
    }

    pub fn update_target(&mut self, bitrate: u32) -> Option<LocalTrackTarget> {
        let new_target = match &self.source {
            Some(ClusterTrackStats::Single { bitrate: _ }) => {
                if bitrate >= SINGLE_STREAM_BASED_BITRATE {
                    LocalTrackTarget::Single { key_only: false }
                } else {
                    LocalTrackTarget::Pause
                }
            }
            Some(ClusterTrackStats::Simulcast { bitrate: _, layers }) => {
                if bitrate < SIMULCAST_BASED_BITRATE {
                    LocalTrackTarget::Pause
                } else {
                    let min_spatial = self.limit.min_spatial.unwrap_or(0);
                    let min_temporal = self.limit.min_temporal.unwrap_or(0);
                    let mut target_spatial = 0;
                    let mut target_temporal = 0;

                    for spatial in 0..(self.limit.max_spatial + 1) {
                        for temporal in 0..(self.limit.max_temporal + 1) {
                            if layers[spatial as usize][temporal as usize] == 0 {
                                break;
                            }
                            if layers[spatial as usize][temporal as usize] <= bitrate || (spatial <= min_spatial && temporal <= min_temporal) {
                                target_spatial = spatial as u8;
                                target_temporal = temporal as u8;
                            } else {
                                break;
                            }
                        }
                    }

                    LocalTrackTarget::Scalable {
                        spatial: target_spatial,
                        temporal: target_temporal,
                        key_only: false,
                    }
                }
            }
            Some(ClusterTrackStats::Svc { bitrate: _, layers }) => {
                if bitrate < SVC_BASED_BITRATE {
                    LocalTrackTarget::Pause
                } else {
                    let min_spatial = self.limit.min_spatial.unwrap_or(0);
                    let min_temporal = self.limit.min_temporal.unwrap_or(0);
                    let mut target_spatial = 0;
                    let mut target_temporal = 0;

                    for spatial in 0..(self.limit.max_spatial + 1) {
                        for temporal in 0..(self.limit.max_temporal + 1) {
                            if layers[spatial as usize][temporal as usize] == 0 {
                                break;
                            }
                            if layers[spatial as usize][temporal as usize] <= bitrate || (spatial <= min_spatial && temporal <= min_temporal) {
                                target_spatial = spatial as u8;
                                target_temporal = temporal as u8;
                            } else {
                                break;
                            }
                        }
                    }

                    LocalTrackTarget::Scalable {
                        spatial: target_spatial,
                        temporal: target_temporal,
                        key_only: false,
                    }
                }
            }
            None => {
                // TODO optimize this, we need to avoid pause track when there is no source, this make slow to start remote stream
                LocalTrackTarget::WaitStart
            }
        };

        if self.target != Some(new_target.clone()) {
            self.target = Some(new_target.clone());
            Some(new_target)
        } else {
            None
        }
    }
}

pub struct BitrateAllocator {
    send_bps: u32,
    tracks: Vec<TrackSlot>,
    out_actions: VecDeque<BitrateAllocationAction>,
}

impl BitrateAllocator {
    pub fn new(send_bps: u32) -> Self {
        Self {
            send_bps,
            tracks: Default::default(),
            out_actions: Default::default(),
        }
    }

    pub fn tick(&mut self) {
        self.refresh();
    }

    pub fn set_est_bitrate(&mut self, bps: u32) {
        self.send_bps = bps;
        self.refresh();
    }

    pub fn add_local_track(&mut self, track: TrackId, priority: u16) {
        log::info!("[BitrateAllocator] add track {} priority {}", track, priority);
        //remove if already has
        self.tracks.retain(|slot| slot.track_id != track);
        self.tracks.push(TrackSlot {
            track_id: track,
            source: None,
            limit: ReceiverLayerLimit {
                priority,
                min_spatial: None,
                max_spatial: 2,
                min_temporal: None,
                max_temporal: 2,
            },
            target: None,
        });
        self.tracks.sort_by_key(|t| t.priority());
    }

    pub fn update_local_track_limit(&mut self, track: TrackId, limit: ReceiverLayerLimit) {
        log::info!("[BitrateAllocator] update track {} limit {:?}", track, limit);
        //finding which track to update
        self.tracks.iter_mut().find(|slot| slot.track_id == track).map(|slot| slot.limit = limit);
        self.tracks.sort_by_key(|t| t.priority());
    }

    pub fn remove_local_track(&mut self, track: TrackId) {
        log::info!("[BitrateAllocator] remove track {}", track);
        self.tracks.retain(|slot| slot.track_id != track);
    }

    pub fn update_source_bitrate(&mut self, track: TrackId, stats: ClusterTrackStats) {
        self.tracks.iter_mut().find(|slot| slot.track_id == track).map(|slot| slot.source = Some(stats));
    }

    fn refresh(&mut self) {
        let mut used_bitrate = 0;
        let mut track_bitrates: HashMap<TrackId, u32> = Default::default();
        let mut sum_priority = 0;

        for track in &self.tracks {
            used_bitrate += track.based_bitrate();
            sum_priority += track.priority() as u32;
            track_bitrates.insert(track.track_id, track.based_bitrate());
            if used_bitrate > self.send_bps {
                break;
            }
        }

        if sum_priority > 0 && self.send_bps > used_bitrate {
            let remain_bitrate = self.send_bps - used_bitrate;
            for track in &self.tracks {
                if let Some(bitrate) = track_bitrates.get_mut(&track.track_id) {
                    *bitrate += remain_bitrate * (track.priority() as u32) / sum_priority;
                }
            }
        }

        for track in self.tracks.iter_mut() {
            if let Some(bitrate) = track_bitrates.get(&track.track_id) {
                if let Some(target) = track.update_target(*bitrate) {
                    self.out_actions.push_back(BitrateAllocationAction::LimitLocalTrack(track.track_id, target));
                }
                self.out_actions.push_back(BitrateAllocationAction::LimitLocalTrackBitrate(track.track_id, *bitrate));
            }
        }
    }

    pub fn pop_action(&mut self) -> Option<BitrateAllocationAction> {
        self.out_actions.pop_back()
    }
}

#[cfg(test)]
mod tests {
    use cluster::ClusterTrackStats;
    use transport::TrackId;

    use crate::{endpoint_wrap::internal::DEFAULT_BITRATE_OUT_BPS, rpc::ReceiverLayerLimit};

    use super::{BitrateAllocationAction, BitrateAllocator, LocalTrackTarget, SINGLE_STREAM_BASED_BITRATE};

    fn create_receiver_limit(priority: u16, max_spatial: u8, max_temporal: u8) -> ReceiverLayerLimit {
        ReceiverLayerLimit {
            priority,
            min_spatial: None,
            max_spatial,
            min_temporal: None,
            max_temporal,
        }
    }

    fn create_receiver_limit_full(priority: u16, max_spatial: u8, max_temporal: u8, min_spatial: u8, min_temporal: u8) -> ReceiverLayerLimit {
        ReceiverLayerLimit {
            priority,
            min_spatial: Some(min_spatial),
            max_spatial,
            min_temporal: Some(min_temporal),
            max_temporal,
        }
    }

    enum Data {
        Tick,
        SetEstBitrate(u32),
        AddLocalTrack(TrackId, u16),
        UpdateLocalTrack(TrackId, ReceiverLayerLimit),
        RemoveLocalTrack(TrackId),
        UpdateSourceBitrate(TrackId, ClusterTrackStats),
        Output(Option<BitrateAllocationAction>),
    }

    fn test(default_send: u32, data: Vec<Data>) {
        let mut allocator = BitrateAllocator::new(default_send);

        let mut index = 0;
        for row in data {
            index += 1;
            match row {
                Data::SetEstBitrate(bps) => allocator.set_est_bitrate(bps),
                Data::Tick => allocator.tick(),
                Data::AddLocalTrack(track, priority) => allocator.add_local_track(track, priority),
                Data::UpdateLocalTrack(track, limit) => allocator.update_local_track_limit(track, limit),
                Data::RemoveLocalTrack(track) => allocator.remove_local_track(track),
                Data::UpdateSourceBitrate(track, stats) => allocator.update_source_bitrate(track, stats),
                Data::Output(expected) => assert_eq!(allocator.pop_action(), expected, "Wrong in row {}", index),
            }
        }
    }

    #[test]
    fn single_track() {
        test(
            DEFAULT_BITRATE_OUT_BPS,
            vec![
                Data::AddLocalTrack(1, 100),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, DEFAULT_BITRATE_OUT_BPS))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(1, LocalTrackTarget::WaitStart))),
                Data::Output(None),
                Data::UpdateSourceBitrate(1, ClusterTrackStats::Single { bitrate: 100_000 }),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, DEFAULT_BITRATE_OUT_BPS))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(1, LocalTrackTarget::Single { key_only: false }))),
                Data::Output(None),
                Data::RemoveLocalTrack(1),
                Data::Tick,
                Data::Output(None),
            ],
        );
    }

    #[test]
    fn multi_track() {
        test(
            DEFAULT_BITRATE_OUT_BPS,
            vec![
                Data::AddLocalTrack(1, 100),
                Data::AddLocalTrack(2, 300),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(
                    2,
                    SINGLE_STREAM_BASED_BITRATE + (DEFAULT_BITRATE_OUT_BPS - SINGLE_STREAM_BASED_BITRATE * 2) * 3 / 4,
                ))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(2, LocalTrackTarget::WaitStart))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(
                    1,
                    SINGLE_STREAM_BASED_BITRATE + (DEFAULT_BITRATE_OUT_BPS - SINGLE_STREAM_BASED_BITRATE * 2) * 1 / 4,
                ))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(1, LocalTrackTarget::WaitStart))),
                Data::Output(None),
                Data::UpdateLocalTrack(1, create_receiver_limit(300, 2, 2)),
                Data::UpdateLocalTrack(2, create_receiver_limit(100, 2, 2)),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(
                    1,
                    SINGLE_STREAM_BASED_BITRATE + (DEFAULT_BITRATE_OUT_BPS - SINGLE_STREAM_BASED_BITRATE * 2) * 3 / 4,
                ))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(
                    2,
                    SINGLE_STREAM_BASED_BITRATE + (DEFAULT_BITRATE_OUT_BPS - SINGLE_STREAM_BASED_BITRATE * 2) * 1 / 4,
                ))),
                Data::Output(None),
            ],
        );
    }

    #[test]
    fn simulcast_single_track() {
        test(
            DEFAULT_BITRATE_OUT_BPS,
            vec![
                Data::AddLocalTrack(1, 100),
                Data::UpdateSourceBitrate(
                    1,
                    ClusterTrackStats::Simulcast {
                        bitrate: 100000,
                        layers: [[100_000, 150_000, 200_000], [200_000, 300_000, 400_000], [400_000, 600_000, 800_000]],
                    },
                ),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, DEFAULT_BITRATE_OUT_BPS))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(
                    1,
                    LocalTrackTarget::Scalable {
                        spatial: 2,
                        temporal: 2,
                        key_only: false,
                    },
                ))),
                Data::Output(None),
                // update for using min_spatial
                Data::UpdateLocalTrack(1, create_receiver_limit_full(100, 2, 2, 1, 1)),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, DEFAULT_BITRATE_OUT_BPS))),
                Data::Output(None),
                Data::SetEstBitrate(100_000),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, 100_000))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(
                    1,
                    LocalTrackTarget::Scalable {
                        spatial: 1,
                        temporal: 1,
                        key_only: false,
                    },
                ))),
                Data::Output(None),
                // update for using limit max_spatial
                Data::UpdateLocalTrack(1, create_receiver_limit_full(100, 0, 0, 0, 0)),
                Data::SetEstBitrate(1_000_000),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, 1_000_000))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(
                    1,
                    LocalTrackTarget::Scalable {
                        spatial: 0,
                        temporal: 0,
                        key_only: false,
                    },
                ))),
                Data::Output(None),
                Data::RemoveLocalTrack(1),
                Data::Tick,
                Data::Output(None),
            ],
        );
    }

    #[test]
    fn simulcast_min_spatial_overwrite() {
        test(
            100000,
            vec![
                Data::AddLocalTrack(1, 100),
                Data::UpdateSourceBitrate(
                    1,
                    ClusterTrackStats::Simulcast {
                        bitrate: 100000,
                        layers: [[100_000, 150_000, 200_000], [200_000, 300_000, 400_000], [0, 0, 0]],
                    },
                ),
                Data::UpdateLocalTrack(1, create_receiver_limit_full(100, 2, 2, 2, 2)),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, 100000))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(
                    1,
                    LocalTrackTarget::Scalable {
                        spatial: 1,
                        temporal: 2,
                        key_only: false,
                    },
                ))),
                Data::Output(None),
                Data::UpdateLocalTrack(1, create_receiver_limit_full(100, 2, 0, 2, 0)),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, 100000))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(
                    1,
                    LocalTrackTarget::Scalable {
                        spatial: 1,
                        temporal: 0,
                        key_only: false,
                    },
                ))),
                Data::Output(None),
            ],
        );
    }

    #[test]
    fn svc_min_spatial_overwrite() {
        test(
            100000,
            vec![
                Data::AddLocalTrack(1, 100),
                Data::UpdateSourceBitrate(
                    1,
                    ClusterTrackStats::Svc {
                        bitrate: 100000,
                        layers: [[100_000, 150_000, 200_000], [200_000, 300_000, 400_000], [0, 0, 0]],
                    },
                ),
                Data::UpdateLocalTrack(1, create_receiver_limit_full(100, 2, 2, 2, 2)),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, 100000))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(
                    1,
                    LocalTrackTarget::Scalable {
                        spatial: 1,
                        temporal: 2,
                        key_only: false,
                    },
                ))),
                Data::Output(None),
                Data::UpdateLocalTrack(1, create_receiver_limit_full(100, 2, 0, 2, 0)),
                Data::Tick,
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrackBitrate(1, 100000))),
                Data::Output(Some(BitrateAllocationAction::LimitLocalTrack(
                    1,
                    LocalTrackTarget::Scalable {
                        spatial: 1,
                        temporal: 0,
                        key_only: false,
                    },
                ))),
                Data::Output(None),
            ],
        );
    }
}
