use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use media_server_protocol::endpoint::{PeerId, RoomId};

use crate::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRemoteTrackEvent},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportEvent, TransportInput, TransportState, TransportStats},
};

use self::{local_track::EndpointLocalTrack, remote_track::EndpointRemoteTrack};

use super::{middleware::EndpointMiddleware, EndpointEvent, EndpointReq, EndpointReqId, EndpointRes};

mod local_track;
mod remote_track;

pub enum InternalOutput {
    Event(EndpointEvent),
    RpcRes(EndpointReqId, EndpointRes),
    Cluster(ClusterEndpointControl),
}

pub struct EndpointInternal {
    state: TransportState,
    room: Option<(RoomId, PeerId)>,
    local_tracks_id: Vec<LocalTrackId>,
    remote_tracks_id: Vec<RemoteTrackId>,
    local_tracks: smallmap::Map<LocalTrackId, EndpointLocalTrack>,
    remote_tracks: smallmap::Map<RemoteTrackId, EndpointRemoteTrack>,
    middlewares: Vec<Box<dyn EndpointMiddleware>>,
    queue: VecDeque<InternalOutput>,
}

impl EndpointInternal {
    pub fn new() -> Self {
        Self {
            state: TransportState::Connecting,
            room: None,
            local_tracks_id: Default::default(),
            remote_tracks_id: Default::default(),
            local_tracks: Default::default(),
            remote_tracks: Default::default(),
            middlewares: Default::default(),
            queue: Default::default(),
        }
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        None
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        self.queue.pop_front()
    }
}

/// This block is for processing transport related event
impl EndpointInternal {
    pub fn on_transport_event<'a>(&mut self, now: Instant, event: TransportEvent) -> Option<InternalOutput> {
        match event {
            TransportEvent::State(state) => self.on_transport_state_changed(now, state),
            TransportEvent::RemoteTrack(track, event) => self.on_transport_remote_track(now, track, event),
            TransportEvent::LocalTrack(track, event) => self.on_transport_local_track(now, track, event),
            TransportEvent::Stats(stats) => self.on_transport_stats(now, stats),
        }
    }

    pub fn on_transport_rpc<'a>(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointReq) -> Option<InternalOutput> {
        match req {
            EndpointReq::JoinRoom(room, peer) => {
                self.room = Some((room.clone(), peer.clone()));
                if matches!(self.state, TransportState::Connecting) {
                    Some(InternalOutput::Cluster(ClusterEndpointControl::JoinRoom(room, peer)))
                } else {
                    None
                }
            }
            EndpointReq::LeaveRoom => {
                self.room.take()?;
                if matches!(self.state, TransportState::Connecting) {
                    Some(InternalOutput::Cluster(ClusterEndpointControl::LeaveRoom))
                } else {
                    None
                }
            }
            EndpointReq::RemoteTrack(track, control) => todo!(),
            EndpointReq::LocalTrack(_, _) => todo!(),
        }
    }

    fn on_transport_state_changed<'a>(&mut self, now: Instant, state: TransportState) -> Option<InternalOutput> {
        self.state = state;
        match &self.state {
            TransportState::Connecting => None,
            TransportState::ConnectError(_) => None,
            TransportState::Connected => {
                for i in 0..self.local_tracks_id.len() {
                    let id = self.local_tracks_id[i];
                    if let Some(out) = self.local_tracks.get_mut(&id).expect("Should have").on_connected(now) {
                        if let Some(out) = self.on_local_track_output(now, id, out) {
                            self.queue.push_back(out);
                        }
                    }
                }
                for i in 0..self.remote_tracks_id.len() {
                    let id = self.remote_tracks_id[i];
                    let track = self.remote_tracks.get_mut(&id).expect("Should have");
                    if let Some(out) = track.on_connected(now) {
                        if let Some(out) = self.on_remote_track_output(now, id, out) {
                            self.queue.push_back(out);
                        }
                    }
                }
                let (room, peer) = self.room.as_ref()?;
                self.queue.push_back(InternalOutput::Cluster(ClusterEndpointControl::JoinRoom(room.clone(), peer.clone())));
                self.queue.pop_front()
            }
            TransportState::Reconnecting => None,
            TransportState::Disconnected(_) => None,
        }
    }

    fn on_transport_remote_track<'a>(&mut self, now: Instant, track: RemoteTrackId, event: RemoteTrackEvent) -> Option<InternalOutput> {
        if event.need_create() {
            self.remote_tracks_id.push(track);
            self.remote_tracks.insert(track, EndpointRemoteTrack::default());
        }
        let out = self.remote_tracks.get_mut(&track)?.on_event(now, event)?;
        self.on_remote_track_output(now, track, out)
    }

    fn on_transport_local_track<'a>(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) -> Option<InternalOutput> {
        if event.need_create() {
            self.local_tracks_id.push(track);
            self.local_tracks.insert(track, EndpointLocalTrack::default());
        }
        let out = self.local_tracks.get_mut(&track)?.on_event(now, event)?;
        self.on_local_track_output(now, track, out)
    }

    fn on_transport_stats<'a>(&mut self, now: Instant, stats: TransportStats) -> Option<InternalOutput> {
        None
    }
}

/// This block is for cluster related events
impl EndpointInternal {
    pub fn on_cluster_event<'a>(&mut self, now: Instant, event: ClusterEndpointEvent) -> Option<InternalOutput> {
        match event {
            ClusterEndpointEvent::PeerJoined(peer) => Some(InternalOutput::Event(EndpointEvent::PeerJoined(peer))),
            ClusterEndpointEvent::PeerLeaved(peer) => Some(InternalOutput::Event(EndpointEvent::PeerLeaved(peer))),
            ClusterEndpointEvent::TrackStarted(peer, track, meta) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStarted(peer, track, meta))),
            ClusterEndpointEvent::TrackStoped(peer, track) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStopped(peer, track))),
            ClusterEndpointEvent::RemoteTrack(track, event) => self.on_cluster_remote_track(now, track, event),
            ClusterEndpointEvent::LocalTrack(track, event) => self.on_cluster_local_track(now, track, event),
        }
    }

    fn on_cluster_remote_track<'a>(&mut self, now: Instant, id: RemoteTrackId, event: ClusterRemoteTrackEvent) -> Option<InternalOutput> {
        match event {
            _ => todo!(),
        }
    }

    fn on_cluster_local_track<'a>(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) -> Option<InternalOutput> {
        match event {
            _ => todo!(),
        }
    }
}

/// This block for internal local and remote track
impl EndpointInternal {
    fn on_remote_track_output<'a>(&mut self, now: Instant, id: RemoteTrackId, out: remote_track::Output) -> Option<InternalOutput> {
        todo!()
    }

    fn on_local_track_output<'a>(&mut self, now: Instant, id: LocalTrackId, out: local_track::Output) -> Option<InternalOutput> {
        todo!()
    }
}
