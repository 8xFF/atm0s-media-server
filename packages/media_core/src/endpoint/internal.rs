//! EndpointInternal compose small parts: local track, remote track. It act as integration hub

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::endpoint::{PeerId, RoomId};
use media_server_utils::Small2dMap;
use sans_io_runtime::{TaskGroup, TaskSwitcher};

use crate::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRemoteTrackEvent, ClusterRoomHash, ClusterRoomInfoPublishLevel, ClusterRoomInfoSubscribeLevel},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportEvent, TransportState, TransportStats},
};

use self::{local_track::EndpointLocalTrack, remote_track::EndpointRemoteTrack};

use super::{middleware::EndpointMiddleware, EndpointEvent, EndpointReq, EndpointReqId, EndpointRes};

mod local_track;
mod remote_track;

#[derive(num_enum::TryFromPrimitive)]
#[repr(usize)]
enum TaskType {
    LocalTracks,
    RemoteTracks,
}

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
    local_tracks_id: Small2dMap<LocalTrackId, usize>,
    remote_tracks_id: Small2dMap<RemoteTrackId, usize>,
    local_tracks: TaskGroup<local_track::Input, local_track::Output, EndpointLocalTrack, 4>,
    remote_tracks: TaskGroup<remote_track::Input, remote_track::Output, EndpointRemoteTrack, 16>,
    _middlewares: Vec<Box<dyn EndpointMiddleware>>,
    queue: VecDeque<InternalOutput>,
    switcher: TaskSwitcher,
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
            _middlewares: Default::default(),
            queue: Default::default(),
            switcher: TaskSwitcher::new(2),
        }
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        loop {
            match self.switcher.looper_current(now)?.try_into().ok()? {
                TaskType::LocalTracks => {
                    if let Some((index, out)) = self.switcher.looper_process(self.local_tracks.on_tick(now)) {
                        let track_id = self.local_tracks_id.get2(&index).expect("Should have local_track_id");
                        if let Some(out) = self.convert_local_track_output(now, *track_id, out) {
                            return Some(out);
                        }
                    }
                }
                TaskType::RemoteTracks => {
                    if let Some((index, out)) = self.switcher.looper_process(self.remote_tracks.on_tick(now)) {
                        let track_id = self.remote_tracks_id.get2(&index).expect("Should have remote_track_id");
                        if let Some(out) = self.convert_remote_track_output(now, *track_id, out) {
                            return Some(out);
                        }
                    }
                }
            }
        }
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }

        loop {
            match self.switcher.queue_current()?.try_into().ok()? {
                TaskType::LocalTracks => {
                    if let Some((index, out)) = self.switcher.queue_process(self.local_tracks.pop_output(now)) {
                        let track_id = self.local_tracks_id.get2(&index).expect("Should have local_track_id");
                        if let Some(out) = self.convert_local_track_output(now, *track_id, out) {
                            return Some(out);
                        }
                    }
                }
                TaskType::RemoteTracks => {
                    if let Some((index, out)) = self.switcher.queue_process(self.remote_tracks.pop_output(now)) {
                        let track_id = self.remote_tracks_id.get2(&index).expect("Should have remote_track_id");
                        if let Some(out) = self.convert_remote_track_output(now, *track_id, out) {
                            return Some(out);
                        }
                    }
                }
            }
        }
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
                let index = self.remote_tracks_id.get1(&track_id)?;
                let out = self.remote_tracks.on_event(now, *index, remote_track::Input::RpcReq(req_id, req))?;
                self.convert_remote_track_output(now, track_id, out)
            }
            EndpointReq::LocalTrack(track_id, req) => {
                let index = self.local_tracks_id.get1(&track_id)?;
                let out = self.local_tracks.on_event(now, *index, local_track::Input::RpcReq(req_id, req))?;
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
                log::info!("[EndpointInternal] connect error {:?}", err);
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
                if let Some(out) = self.leave_room(now) {
                    self.queue.push_back(out);
                }
                self.queue.push_back(InternalOutput::Destroy);
                self.queue.pop_front()
            }
        }
    }

    fn on_transport_remote_track<'a>(&mut self, now: Instant, track: RemoteTrackId, event: RemoteTrackEvent) -> Option<InternalOutput> {
        if let Some(meta) = event.need_create() {
            log::info!("[EndpointInternal] create remote track {:?}", track);
            let room = self.joined.as_ref().map(|j| j.0.clone());
            let index = self.remote_tracks.add_task(EndpointRemoteTrack::new(room, meta));
            self.remote_tracks_id.insert(track, index);
        }
        let index = self.remote_tracks_id.get1(&track)?;
        let out = self.remote_tracks.on_event(now, *index, remote_track::Input::Event(event))?;
        self.convert_remote_track_output(now, track, out)
    }

    fn on_transport_local_track<'a>(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) -> Option<InternalOutput> {
        if event.need_create() {
            log::info!("[EndpointInternal] create local track {:?}", track);
            let room = self.joined.as_ref().map(|j| j.0.clone());
            let index = self.local_tracks.add_task(EndpointLocalTrack::new(room));
            self.local_tracks_id.insert(track, index);
        }
        let index = self.local_tracks_id.get1(&track)?;
        let out = self.local_tracks.on_event(now, *index, local_track::Input::Event(event))?;
        self.convert_local_track_output(now, track, out)
    }

    fn on_transport_stats<'a>(&mut self, _now: Instant, _stats: TransportStats) -> Option<InternalOutput> {
        None
    }

    fn join_room<'a>(&mut self, now: Instant, room: RoomId, peer: PeerId) -> Option<InternalOutput> {
        let room_hash: ClusterRoomHash = (&room).into();
        log::info!("[EndpointInternal] join_room({room}, {peer}), room_hash {room_hash}");

        if let Some(out) = self.leave_room(now) {
            self.queue.push_back(out);
        }

        self.joined = Some(((&room).into(), room.clone(), peer.clone()));
        self.queue.push_back(InternalOutput::Cluster(
            (&room).into(),
            ClusterEndpointControl::Join(peer, ClusterRoomInfoPublishLevel::Full, ClusterRoomInfoSubscribeLevel::Full),
        ));

        for (track_id, index) in self.local_tracks_id.pairs() {
            if let Some(out) = self.local_tracks.on_event(now, index, local_track::Input::JoinRoom(room_hash)) {
                if let Some(out) = self.convert_local_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

        for (track_id, index) in self.remote_tracks_id.pairs() {
            if let Some(out) = self.remote_tracks.on_event(now, index, remote_track::Input::JoinRoom(room_hash)) {
                if let Some(out) = self.convert_remote_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

        self.queue.pop_front()
    }

    fn leave_room<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        let (hash, room, peer) = self.joined.take()?;
        log::info!("[EndpointInternal] leave_room({room}, {peer})");

        for (track_id, index) in self.local_tracks_id.pairs() {
            if let Some(out) = self.local_tracks.on_event(now, index, local_track::Input::LeaveRoom) {
                if let Some(out) = self.convert_local_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

        for (track_id, index) in self.remote_tracks_id.pairs() {
            if let Some(out) = self.remote_tracks.on_event(now, index, remote_track::Input::LeaveRoom) {
                if let Some(out) = self.convert_remote_track_output(now, track_id, out) {
                    self.queue.push_back(out);
                }
            }
        }

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
        let index = self.remote_tracks_id.get1(&id)?;
        let out = self.remote_tracks.on_event(now, *index, remote_track::Input::Cluster(event))?;
        self.convert_remote_track_output(now, id, out)
    }

    fn on_cluster_local_track<'a>(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) -> Option<InternalOutput> {
        let index = self.local_tracks_id.get1(&id)?;
        let out = self.local_tracks.on_event(now, *index, local_track::Input::Cluster(event))?;
        self.convert_local_track_output(now, id, out)
    }
}

/// This block for internal local and remote track
impl EndpointInternal {
    fn convert_remote_track_output<'a>(&mut self, _now: Instant, id: RemoteTrackId, out: remote_track::Output) -> Option<InternalOutput> {
        self.switcher.queue_flag_task(TaskType::RemoteTracks as usize);
        match out {
            remote_track::Output::Event(event) => Some(InternalOutput::Event(EndpointEvent::RemoteMediaTrack(id, event))),
            remote_track::Output::Cluster(room, control) => Some(InternalOutput::Cluster(room, ClusterEndpointControl::RemoteTrack(id, control))),
            remote_track::Output::RpcRes(req_id, res) => Some(InternalOutput::RpcRes(req_id, EndpointRes::RemoteTrack(id, res))),
        }
    }

    fn convert_local_track_output<'a>(&mut self, _now: Instant, id: LocalTrackId, out: local_track::Output) -> Option<InternalOutput> {
        self.switcher.queue_flag_task(TaskType::LocalTracks as usize);
        match out {
            local_track::Output::Event(event) => Some(InternalOutput::Event(EndpointEvent::LocalMediaTrack(id, event))),
            local_track::Output::Cluster(room, control) => Some(InternalOutput::Cluster(room, ClusterEndpointControl::LocalTrack(id, control))),
            local_track::Output::RpcRes(req_id, res) => Some(InternalOutput::RpcRes(req_id, EndpointRes::LocalTrack(id, res))),
        }
    }
}

#[cfg(test)]
mod tests {
    //TODO single local track, join leave room
    //TODO multi local tracks, join leave room
    //TODO single remote track, join leave room
    //TODO multi remote tracks, join leave room
    //TODO both local and remote tracks, join leave room
    //TODO handle close request
    //TODO handle transport connected
    //TODO handle transport disconnected
}
