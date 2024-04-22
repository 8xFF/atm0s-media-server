use std::{collections::VecDeque, time::Instant};

use media_server_protocol::endpoint::{PeerId, RoomId};

use crate::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRemoteTrackEvent, ClusterRoomHash},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportEvent, TransportState, TransportStats},
};

use self::{local_track::EndpointLocalTrack, remote_track::EndpointRemoteTrack};

use super::{middleware::EndpointMiddleware, EndpointEvent, EndpointReq, EndpointReqId, EndpointRes};

mod local_track;
mod remote_track;

pub enum InternalOutput {
    Event(EndpointEvent),
    RpcRes(EndpointReqId, EndpointRes),
    Cluster(ClusterRoomHash, ClusterEndpointControl),
    Destroy,
}

pub struct EndpointInternal {
    state: TransportState,
    wait_join: Option<(RoomId, PeerId)>,
    joined: Option<(ClusterRoomHash, RoomId, PeerId)>,
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
            wait_join: None,
            joined: None,
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
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }

        //TODO optimize performance
        for i in 0..self.local_tracks_id.len() {
            let track_id = self.local_tracks_id[i];
            if let Some(out) = self.local_tracks[&track_id].pop_output() {
                if let Some(out) = self.convert_local_track_output(now, track_id, out) {
                    return Some(out);
                }
            }
        }

        //TODO optimize performance
        for i in 0..self.remote_tracks_id.len() {
            let track_id = self.remote_tracks_id[i];
            if let Some(out) = self.remote_tracks[&track_id].pop_output() {
                if let Some(out) = self.convert_remote_track_output(now, track_id, out) {
                    return Some(out);
                }
            }
        }

        None
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
                if matches!(self.state, TransportState::Connecting) {
                    log::info!("[EndpointInternal] join_room({room}, {peer}) but in Connecting state => wait");
                    self.wait_join = Some((room, peer));
                    None
                } else {
                    self.join_room(now, room, peer)
                }
            }
            EndpointReq::LeaveRoom => {
                if let Some((room, peer)) = self.wait_join.take() {
                    log::info!("[EndpointInternal] leave_room({room}, {peer}) but in Connecting state => only clear local");
                    None
                } else {
                    self.leave_room(now)
                }
            }
            EndpointReq::RemoteTrack(track_id, req) => {
                let track = self.remote_tracks.get_mut(&track_id)?;
                let out = track.on_rpc_req(now, req_id, req)?;
                self.convert_remote_track_output(now, track_id, out)
            }
            EndpointReq::LocalTrack(track_id, req) => {
                let track = self.local_tracks.get_mut(&track_id)?;
                let out = track.on_rpc_req(now, req_id, req)?;
                self.convert_local_track_output(now, track_id, out)
            }
        }
    }

    fn on_transport_state_changed<'a>(&mut self, now: Instant, state: TransportState) -> Option<InternalOutput> {
        self.state = state;
        match &self.state {
            TransportState::Connecting => {
                log::info!("[EndpointInternal] connecting");
                None
            }
            TransportState::ConnectError(err) => {
                log::info!("[EndpointInternal] connect error");
                Some(InternalOutput::Destroy)
            }
            TransportState::Connected => {
                log::info!("[EndpointInternal] connected");
                let (room, peer) = self.wait_join.take()?;
                log::info!("[EndpointInternal] join_room({room}, {peer}) after connected");
                self.join_room(now, room, peer)
            }
            TransportState::Reconnecting => {
                log::info!("[EndpointInternal] reconnecting");
                None
            }
            TransportState::Disconnected(err) => {
                log::info!("[EndpointInternal] disconnected {:?}", err);
                if let Some((hash, room, peer)) = &self.joined {
                    log::info!("[EndpointInternal] leave_room({room}, {peer}) after disconnected");
                    self.queue.push_back(InternalOutput::Cluster(*hash, ClusterEndpointControl::Leave));
                }
                self.queue.push_back(InternalOutput::Destroy);
                self.queue.pop_front()
            }
        }
    }

    fn on_transport_remote_track<'a>(&mut self, now: Instant, track: RemoteTrackId, event: RemoteTrackEvent) -> Option<InternalOutput> {
        if let Some(meta) = event.need_create() {
            log::info!("[EndpointInternal] create remote track {:?}", track);
            self.remote_tracks_id.push(track);
            let room = self.joined.as_ref().map(|j| j.0.clone());
            self.remote_tracks.insert(track, EndpointRemoteTrack::new(room, meta));
        }
        let out = self.remote_tracks.get_mut(&track)?.on_transport_event(now, event)?;
        self.convert_remote_track_output(now, track, out)
    }

    fn on_transport_local_track<'a>(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) -> Option<InternalOutput> {
        if event.need_create() {
            log::info!("[EndpointInternal] create local track {:?}", track);
            self.local_tracks_id.push(track);
            let room = self.joined.as_ref().map(|j| j.0.clone());
            self.local_tracks.insert(track, EndpointLocalTrack::new(room));
        }
        let out = self.local_tracks.get_mut(&track)?.on_transport_event(now, event)?;
        self.convert_local_track_output(now, track, out)
    }

    fn on_transport_stats<'a>(&mut self, now: Instant, stats: TransportStats) -> Option<InternalOutput> {
        None
    }

    fn join_room<'a>(&mut self, now: Instant, room: RoomId, peer: PeerId) -> Option<InternalOutput> {
        let room_hash: ClusterRoomHash = (&room).into();
        log::info!("[EndpointInternal] join_room({room}, {peer}), room_hash {room_hash}");

        if let Some(out) = self.leave_room(now) {
            self.queue.push_back(out);
        }

        self.joined = Some(((&room).into(), room.clone(), peer.clone()));
        self.queue.push_back(InternalOutput::Cluster((&room).into(), ClusterEndpointControl::Join(peer)));

        for i in 0..self.local_tracks_id.len() {
            let track_id = self.local_tracks_id[i];
            let track = &mut self.local_tracks[&track_id];
            if let Some(out) = track.on_join_room(now, room_hash) {
                if let Some(out) = self.convert_local_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

        for i in 0..self.remote_tracks_id.len() {
            let track_id = self.remote_tracks_id[i];
            let track = &mut self.remote_tracks[&track_id];
            if let Some(out) = track.on_join_room(now, room_hash) {
                if let Some(out) = self.convert_remote_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

        self.queue.pop_front()
    }

    fn leave_room<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        let (hash, room, peer) = self.joined.take()?;

        for i in 0..self.local_tracks_id.len() {
            let track_id = self.local_tracks_id[i];
            let track = &mut self.local_tracks[&track_id];
            if let Some(out) = track.on_leave_room(now) {
                if let Some(out) = self.convert_local_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

        for i in 0..self.remote_tracks_id.len() {
            let track_id = self.remote_tracks_id[i];
            let track = &mut self.remote_tracks[&track_id];
            if let Some(out) = track.on_leave_room(now) {
                if let Some(out) = self.convert_remote_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

        log::info!("[EndpointInternal] leave_room({room}, {peer})");
        Some(InternalOutput::Cluster(hash, ClusterEndpointControl::Leave))
    }
}

/// This block is for cluster related events
impl EndpointInternal {
    pub fn on_cluster_event<'a>(&mut self, now: Instant, event: ClusterEndpointEvent) -> Option<InternalOutput> {
        match event {
            ClusterEndpointEvent::TrackStarted(peer, track, meta) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStarted(peer, track, meta))),
            ClusterEndpointEvent::TrackStoped(peer, track) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStopped(peer, track))),
            ClusterEndpointEvent::RemoteTrack(track, event) => self.on_cluster_remote_track(now, track, event),
            ClusterEndpointEvent::LocalTrack(track, event) => self.on_cluster_local_track(now, track, event),
        }
    }

    fn on_cluster_remote_track<'a>(&mut self, now: Instant, id: RemoteTrackId, event: ClusterRemoteTrackEvent) -> Option<InternalOutput> {
        let track = self.remote_tracks.get_mut(&id)?;
        let out = track.on_cluster_event(now, event)?;
        self.convert_remote_track_output(now, id, out)
    }

    fn on_cluster_local_track<'a>(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) -> Option<InternalOutput> {
        let track = self.local_tracks.get_mut(&id)?;
        let out = track.on_cluster_event(now, event)?;
        self.convert_local_track_output(now, id, out)
    }
}

/// This block for internal local and remote track
impl EndpointInternal {
    fn convert_remote_track_output<'a>(&mut self, now: Instant, id: RemoteTrackId, out: remote_track::Output) -> Option<InternalOutput> {
        match out {
            remote_track::Output::Event(event) => Some(InternalOutput::Event(EndpointEvent::RemoteMediaTrack(id, event))),
            remote_track::Output::Cluster(room, control) => Some(InternalOutput::Cluster(room, ClusterEndpointControl::RemoteTrack(id, control))),
            remote_track::Output::RpcRes(req_id, res) => Some(InternalOutput::RpcRes(req_id, EndpointRes::RemoteTrack(id, res))),
        }
    }

    fn convert_local_track_output<'a>(&mut self, now: Instant, id: LocalTrackId, out: local_track::Output) -> Option<InternalOutput> {
        match out {
            local_track::Output::Event(event) => Some(InternalOutput::Event(EndpointEvent::LocalMediaTrack(id, event))),
            local_track::Output::Cluster(room, control) => Some(InternalOutput::Cluster(room, ClusterEndpointControl::LocalTrack(id, control))),
            local_track::Output::RpcRes(req_id, res) => Some(InternalOutput::RpcRes(req_id, EndpointRes::LocalTrack(id, res))),
        }
    }
}
