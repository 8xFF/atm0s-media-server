use crate::transport::{LocalTrackId, RemoteTrackId};

use self::{egress::EgressBitrateAllocator, ingress::IngressBitrateAllocator};

mod egress;
mod ingress;

pub use egress::Action as EgressAction;
pub use ingress::Action as IngressAction;
use media_server_protocol::endpoint::TrackPriority;

#[derive(Debug, PartialEq, Eq)]
pub enum Output {
    RemoteTrack(RemoteTrackId, IngressAction),
    LocalTrack(LocalTrackId, EgressAction),
    BweConfig(u64, u64),
}

pub struct BitrateAllocator {
    egress: EgressBitrateAllocator,
    ingress: IngressBitrateAllocator,
}

impl BitrateAllocator {
    pub fn new(max_ingress_bitrate: u64) -> Self {
        Self {
            egress: Default::default(),
            ingress: IngressBitrateAllocator::new(max_ingress_bitrate),
        }
    }

    pub fn on_tick(&mut self) {
        self.egress.on_tick();
        self.ingress.on_tick();
    }

    pub fn set_egress_estimate(&mut self, bitrate: u64) {
        self.egress.set_egress_estimate(bitrate);
    }

    pub fn set_egress_video_track(&mut self, track: LocalTrackId, priority: TrackPriority) {
        self.egress.set_video_track(track, priority);
    }

    pub fn del_egress_video_track(&mut self, track: LocalTrackId) {
        self.egress.del_video_track(track);
    }

    pub fn set_ingress_video_track(&mut self, track: RemoteTrackId, priority: TrackPriority) {
        self.ingress.set_video_track(track, priority);
    }

    pub fn del_ingress_video_track(&mut self, track: RemoteTrackId) {
        self.ingress.del_video_track(track);
    }

    pub fn pop_output(&mut self) -> Option<Output> {
        if let Some(out) = self.egress.pop_output() {
            let out = match out {
                egress::Output::Track(track, action) => Output::LocalTrack(track, action),
                egress::Output::BweConfig(current, desired) => Output::BweConfig(current, desired),
            };
            return Some(out);
        }

        let (track, action) = self.ingress.pop_output()?;
        Some(Output::RemoteTrack(track, action))
    }
}
