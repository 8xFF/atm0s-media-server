use std::time::Instant;

use sans_io_runtime::backend::BackendOutgoing;

use crate::{
    cluster::{ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRemoteTrackEvent},
    transport::{ClientEndpointControl, ClientEndpointEvent, LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportControl, TransportEvent, TransportState, TransportStats},
};

use super::middleware::EndpointMiddleware;

pub enum InternalOutput<'a, Ext> {
    Net(BackendOutgoing<'a>),
    Transport(TransportControl<'a, Ext>),
}

pub struct EndpointInternal {
    middlewares: Vec<Box<dyn EndpointMiddleware>>,
}

impl EndpointInternal {
    pub fn new() -> Self {
        Self { middlewares: Vec::new() }
    }

    pub fn on_tick<'a, ExtIn>(&mut self, now: Instant) -> Option<InternalOutput<'a, ExtIn>> {
        None
    }

    pub fn pop_output<'a, ExtIn>(&mut self, now: Instant) -> Option<InternalOutput<'a, ExtIn>> {
        None
    }

    pub fn shutdown<'a, ExtIn>(&mut self, now: Instant) -> Option<InternalOutput<'a, ExtIn>> {
        None
    }
}

/// This block is for processing transport related event
impl EndpointInternal {
    pub fn on_transport_event<'a, ExtIn, ExtOut>(&mut self, now: Instant, event: TransportEvent<'a, ExtOut>) -> Option<InternalOutput<'a, ExtIn>> {
        match event {
            TransportEvent::Net(out) => Some(InternalOutput::Net(out)),
            TransportEvent::State(state) => self.on_transport_state_changed(now, state),
            TransportEvent::RemoteTrack(track, event) => self.on_transport_remote_track(now, track, event),
            TransportEvent::LocalTrack(track, event) => self.on_transport_local_track(now, track, event),
            TransportEvent::Stats(stats) => self.on_transport_stats(now, stats),
            TransportEvent::Control(control) => self.on_transport_control(now, control),
            TransportEvent::Ext(_) => panic!("should not get here"),
        }
    }

    fn on_transport_state_changed<'a, ExtIn>(&mut self, now: Instant, state: TransportState) -> Option<InternalOutput<'a, ExtIn>> {
        match state {
            TransportState::Connecting => todo!(),
            TransportState::ConnectError(_) => todo!(),
            TransportState::Connected => todo!(),
            TransportState::Reconnecting => todo!(),
            TransportState::Disconnected(_) => todo!(),
        }
    }

    fn on_transport_remote_track<'a, ExtIn>(&mut self, now: Instant, track: RemoteTrackId, event: RemoteTrackEvent) -> Option<InternalOutput<'a, ExtIn>> {
        match event {
            RemoteTrackEvent::Started { name } => todo!(),
            RemoteTrackEvent::Paused => todo!(),
            RemoteTrackEvent::Media(_) => todo!(),
            RemoteTrackEvent::Ended => todo!(),
        }
    }

    fn on_transport_local_track<'a, ExtIn>(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) -> Option<InternalOutput<'a, ExtIn>> {
        match event {
            LocalTrackEvent::Started { name } => todo!(),
            LocalTrackEvent::Paused => todo!(),
            LocalTrackEvent::RequestKeyFrame => todo!(),
            LocalTrackEvent::Ended => todo!(),
        }
    }

    fn on_transport_control<'a, ExtIn>(&mut self, now: Instant, control: ClientEndpointControl) -> Option<InternalOutput<'a, ExtIn>> {
        todo!()
    }

    fn on_transport_stats<'a, ExtIn>(&mut self, now: Instant, stats: TransportStats) -> Option<InternalOutput<'a, ExtIn>> {
        todo!()
    }
}

/// This block is for cluster related events
impl EndpointInternal {
    pub fn on_cluster_event<'a, ExtIn>(&mut self, now: Instant, event: ClusterEndpointEvent) -> Option<InternalOutput<'a, ExtIn>> {
        match event {
            ClusterEndpointEvent::PeerJoined(peer) => Some(InternalOutput::Transport(TransportControl::Event(ClientEndpointEvent::PeerJoined(peer)))),
            ClusterEndpointEvent::PeerLeaved(peer) => Some(InternalOutput::Transport(TransportControl::Event(ClientEndpointEvent::PeerJoined(peer)))),
            ClusterEndpointEvent::TrackStarted(peer, track, meta) => Some(InternalOutput::Transport(TransportControl::Event(ClientEndpointEvent::PeerTrackStarted(peer, track, meta)))),
            ClusterEndpointEvent::TrackStoped(peer, track) => Some(InternalOutput::Transport(TransportControl::Event(ClientEndpointEvent::PeerTrackStopped(peer, track)))),
            ClusterEndpointEvent::RemoteTrack(track, event) => self.on_cluster_remote_track(now, track, event),
            ClusterEndpointEvent::LocalTrack(track, event) => self.on_cluster_local_track(now, track, event),
        }
    }

    fn on_cluster_remote_track<'a, ExtIn>(&mut self, now: Instant, id: RemoteTrackId, event: ClusterRemoteTrackEvent) -> Option<InternalOutput<'a, ExtIn>> {
        match event {
            _ => todo!(),
        }
    }

    fn on_cluster_local_track<'a, ExtIn>(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) -> Option<InternalOutput<'a, ExtIn>> {
        match event {
            _ => todo!(),
        }
    }
}
