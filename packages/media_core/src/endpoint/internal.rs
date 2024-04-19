use std::time::Instant;

use media_server_protocol::endpoint::{PeerId, RoomId};
use sans_io_runtime::backend::BackendOutgoing;

use crate::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRemoteTrackEvent},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportEvent, TransportInput, TransportState, TransportStats},
};

use super::{middleware::EndpointMiddleware, EndpointEvent, EndpointReq, EndpointReqId, EndpointRes};

pub enum InternalOutput<'a> {
    Net(BackendOutgoing<'a>),
    Event(EndpointEvent),
    RpcRes(EndpointReqId, EndpointRes),
    Cluster(ClusterEndpointControl),
}

pub struct EndpointInternal {
    room: Option<(RoomId, PeerId)>,
    middlewares: Vec<Box<dyn EndpointMiddleware>>,
}

impl EndpointInternal {
    pub fn new() -> Self {
        Self { room: None, middlewares: Vec::new() }
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        None
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        None
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        None
    }
}

/// This block is for processing transport related event
impl EndpointInternal {
    pub fn on_transport_event<'a>(&mut self, now: Instant, event: TransportEvent) -> Option<InternalOutput<'a>> {
        match event {
            TransportEvent::State(state) => self.on_transport_state_changed(now, state),
            TransportEvent::RemoteTrack(track, event) => self.on_transport_remote_track(now, track, event),
            TransportEvent::LocalTrack(track, event) => self.on_transport_local_track(now, track, event),
            TransportEvent::Stats(stats) => self.on_transport_stats(now, stats),
        }
    }

    pub fn on_transport_rpc<'a>(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointReq) -> Option<InternalOutput<'a>> {
        todo!()
    }

    fn on_transport_state_changed<'a>(&mut self, now: Instant, state: TransportState) -> Option<InternalOutput<'a>> {
        match state {
            TransportState::Connecting => todo!(),
            TransportState::ConnectError(_) => todo!(),
            TransportState::Connected => todo!(),
            TransportState::Reconnecting => todo!(),
            TransportState::Disconnected(_) => todo!(),
        }
    }

    fn on_transport_remote_track<'a>(&mut self, now: Instant, track: RemoteTrackId, event: RemoteTrackEvent) -> Option<InternalOutput<'a>> {
        match event {
            RemoteTrackEvent::Started { name } => todo!(),
            RemoteTrackEvent::Paused => todo!(),
            RemoteTrackEvent::Resumed => todo!(),
            RemoteTrackEvent::Media(_) => todo!(),
            RemoteTrackEvent::Ended => todo!(),
        }
    }

    fn on_transport_local_track<'a>(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) -> Option<InternalOutput<'a>> {
        match event {
            LocalTrackEvent::RequestKeyFrame => todo!(),
            LocalTrackEvent::Switch(_) => todo!(),
        }
    }

    fn on_transport_req<'a>(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointReq) -> Option<InternalOutput<'a>> {
        match req {
            EndpointReq::JoinRoom(room, peer) => {
                self.room = Some((room.clone(), peer.clone()));
                Some(InternalOutput::Cluster(ClusterEndpointControl::JoinRoom(room, peer)))
            }
            EndpointReq::LeaveRoom => {
                self.room.take()?;
                Some(InternalOutput::Cluster(ClusterEndpointControl::LeaveRoom))
            }
            EndpointReq::RemoteTrack(track, control) => todo!(),
            EndpointReq::LocalTrack(_, _) => todo!(),
        }
    }

    fn on_transport_stats<'a>(&mut self, now: Instant, stats: TransportStats) -> Option<InternalOutput<'a>> {
        todo!()
    }
}

/// This block is for cluster related events
impl EndpointInternal {
    pub fn on_cluster_event<'a>(&mut self, now: Instant, event: ClusterEndpointEvent) -> Option<InternalOutput<'a>> {
        match event {
            ClusterEndpointEvent::PeerJoined(peer) => Some(InternalOutput::Event(EndpointEvent::PeerJoined(peer))),
            ClusterEndpointEvent::PeerLeaved(peer) => Some(InternalOutput::Event(EndpointEvent::PeerLeaved(peer))),
            ClusterEndpointEvent::TrackStarted(peer, track, meta) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStarted(peer, track, meta))),
            ClusterEndpointEvent::TrackStoped(peer, track) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStopped(peer, track))),
            ClusterEndpointEvent::RemoteTrack(track, event) => self.on_cluster_remote_track(now, track, event),
            ClusterEndpointEvent::LocalTrack(track, event) => self.on_cluster_local_track(now, track, event),
        }
    }

    fn on_cluster_remote_track<'a>(&mut self, now: Instant, id: RemoteTrackId, event: ClusterRemoteTrackEvent) -> Option<InternalOutput<'a>> {
        match event {
            _ => todo!(),
        }
    }

    fn on_cluster_local_track<'a>(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) -> Option<InternalOutput<'a>> {
        match event {
            _ => todo!(),
        }
    }
}
