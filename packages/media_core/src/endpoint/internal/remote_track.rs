use std::time::Instant;

use media_server_protocol::endpoint::TrackName;

use crate::{cluster::ClusterRemoteTrackControl, transport::RemoteTrackEvent};

pub enum Output {
    Cluster(ClusterRemoteTrackControl),
}

#[derive(Default)]
pub struct EndpointRemoteTrack {}

impl EndpointRemoteTrack {
    pub fn on_connected(&mut self, now: Instant) -> Option<Output> {
        None
    }

    pub fn on_transport_event(&mut self, now: Instant, event: RemoteTrackEvent) -> Option<Output> {
        match event {
            RemoteTrackEvent::Started { name } => Some(Output::Cluster(ClusterRemoteTrackControl::Started(TrackName(name)))),
            RemoteTrackEvent::Paused => None,
            RemoteTrackEvent::Resumed => None,
            RemoteTrackEvent::Media(_) => None,
            RemoteTrackEvent::Ended => Some(Output::Cluster(ClusterRemoteTrackControl::Ended)),
        }
    }

    pub fn pop_output(&mut self) -> Option<Output> {
        None
    }
}
