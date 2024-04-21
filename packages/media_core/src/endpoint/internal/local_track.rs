use std::time::Instant;

use crate::{
    cluster::ClusterLocalTrackControl,
    transport::{LocalTrackEvent, LocalTrackId},
};

pub enum Output {
    Cluster(ClusterLocalTrackControl),
}

#[derive(Default)]
pub struct EndpointLocalTrack {}

impl EndpointLocalTrack {
    pub fn on_connected(&mut self, now: Instant) -> Option<Output> {
        None
    }
    pub fn on_transport_event(&mut self, now: Instant, event: LocalTrackEvent) -> Option<Output> {
        log::info!("[EndpointLocalTrack] on event {:?}", event);
        match event {
            LocalTrackEvent::Started => None,
            //TODO maybe switch is RPC type
            LocalTrackEvent::Switch(Some((peer, track))) => Some(Output::Cluster(ClusterLocalTrackControl::Subscribe(peer, track))),
            LocalTrackEvent::Switch(None) => Some(Output::Cluster(ClusterLocalTrackControl::Unsubscribe)),
            LocalTrackEvent::RequestKeyFrame => Some(Output::Cluster(ClusterLocalTrackControl::RequestKeyFrame)),
            LocalTrackEvent::Ended => None,
        }
    }
    pub fn pop_output(&mut self) -> Option<Output> {
        None
    }
}
