//! EndpointInternal compose small parts: local track, remote track. It act as integration hub

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe},
    transport::RpcError,
};
use media_server_utils::Small2dMap;
use sans_io_runtime::{return_if_none, return_if_some, TaskGroup, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRemoteTrackEvent, ClusterRoomHash},
    errors::EndpointErrors,
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportEvent, TransportState, TransportStats},
};

use self::{bitrate_allocator::BitrateAllocator, local_track::EndpointLocalTrack, remote_track::EndpointRemoteTrack};

use super::{middleware::EndpointMiddleware, EndpointCfg, EndpointEvent, EndpointReq, EndpointReqId, EndpointRes};

mod bitrate_allocator;
mod local_track;
mod remote_track;

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(usize)]
enum TaskType {
    LocalTracks,
    RemoteTracks,
    BitrateAllocator,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InternalOutput {
    Event(EndpointEvent),
    RpcRes(EndpointReqId, EndpointRes),
    Cluster(ClusterRoomHash, ClusterEndpointControl),
    Destroy,
}

pub struct EndpointInternal {
    cfg: EndpointCfg,
    state: TransportState,
    wait_join: Option<(EndpointReqId, RoomId, PeerId, PeerMeta, RoomInfoPublish, RoomInfoSubscribe)>,
    joined: Option<(ClusterRoomHash, RoomId, PeerId)>,
    local_tracks_id: Small2dMap<LocalTrackId, usize>,
    remote_tracks_id: Small2dMap<RemoteTrackId, usize>,
    local_tracks: TaskSwitcherBranch<TaskGroup<local_track::Input, local_track::Output, EndpointLocalTrack, 4>, (usize, local_track::Output)>,
    remote_tracks: TaskSwitcherBranch<TaskGroup<remote_track::Input, remote_track::Output, EndpointRemoteTrack, 16>, (usize, remote_track::Output)>,
    bitrate_allocator: TaskSwitcherBranch<BitrateAllocator, bitrate_allocator::Output>,
    _middlewares: Vec<Box<dyn EndpointMiddleware>>,
    queue: VecDeque<InternalOutput>,
    switcher: TaskSwitcher,
}

impl EndpointInternal {
    pub fn new(cfg: EndpointCfg) -> Self {
        Self {
            state: TransportState::Connecting,
            wait_join: None,
            joined: None,
            local_tracks_id: Default::default(),
            remote_tracks_id: Default::default(),
            local_tracks: TaskSwitcherBranch::default(TaskType::LocalTracks),
            remote_tracks: TaskSwitcherBranch::default(TaskType::RemoteTracks),
            bitrate_allocator: TaskSwitcherBranch::new(BitrateAllocator::new(cfg.max_ingress_bitrate, cfg.max_ingress_bitrate), TaskType::BitrateAllocator),
            _middlewares: Default::default(),
            queue: Default::default(),
            switcher: TaskSwitcher::new(3),
            cfg,
        }
    }

    pub fn on_tick<'a>(&mut self, now: Instant) {
        self.bitrate_allocator.input(&mut self.switcher).on_tick();
        self.local_tracks.input(&mut self.switcher).on_tick(now);
        self.remote_tracks.input(&mut self.switcher).on_tick(now);
    }
}

impl TaskSwitcherChild<InternalOutput> for EndpointInternal {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<InternalOutput> {
        return_if_some!(self.queue.pop_front());

        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::BitrateAllocator => self.pop_bitrate_allocator(now),
                TaskType::LocalTracks => self.pop_local_tracks(now),
                TaskType::RemoteTracks => self.pop_remote_tracks(now),
            }
            return_if_some!(self.queue.pop_front());
        }
    }
}

/// This block is for processing transport related event
impl EndpointInternal {
    pub fn on_transport_event(&mut self, now: Instant, event: TransportEvent) {
        match event {
            TransportEvent::State(state) => self.on_transport_state_changed(now, state),
            TransportEvent::RemoteTrack(track, event) => self.on_transport_remote_track(now, track, event),
            TransportEvent::LocalTrack(track, event) => self.on_transport_local_track(now, track, event),
            TransportEvent::Stats(stats) => self.on_transport_stats(now, stats),
            TransportEvent::EgressBitrateEstimate(bitrate) => {
                let bitrate2 = bitrate.min(self.cfg.max_egress_bitrate as u64);
                log::debug!("[EndpointInternal] limit egress bitrate {bitrate2}, rewrite from {bitrate}");
                self.bitrate_allocator.input(&mut self.switcher).set_egress_estimate(bitrate2);
            }
        }
    }

    pub fn on_transport_rpc<'a>(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointReq) {
        match req {
            EndpointReq::JoinRoom(room, peer, meta, publish, subscribe) => {
                if matches!(self.state, TransportState::Connecting) {
                    log::info!("[EndpointInternal] join_room({room}, {peer}) but in Connecting state => wait");
                    self.wait_join = Some((req_id, room, peer, meta, publish, subscribe));
                } else {
                    self.join_room(now, req_id, room, peer, meta, publish, subscribe);
                }
            }
            EndpointReq::LeaveRoom => {
                if let Some((_req_id, room, peer, _meta, _publish, _subscribe)) = self.wait_join.take() {
                    log::info!("[EndpointInternal] leave_room({room}, {peer}) but in Connecting state => only clear local");
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::LeaveRoom(Ok(()))));
                } else {
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::LeaveRoom(Ok(()))));
                    self.leave_room(now);
                }
            }
            EndpointReq::SubscribePeer(peer) => {
                if let Some((room, _, _)) = &self.joined {
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::SubscribePeer(Ok(()))));
                    self.queue.push_back(InternalOutput::Cluster(*room, ClusterEndpointControl::SubscribePeer(peer)));
                } else {
                    self.queue
                        .push_back(InternalOutput::RpcRes(req_id, EndpointRes::SubscribePeer(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))));
                }
            }
            EndpointReq::UnsubscribePeer(peer) => {
                if let Some((room, _, _)) = &self.joined {
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::UnsubscribePeer(Ok(()))));
                    self.queue.push_back(InternalOutput::Cluster(*room, ClusterEndpointControl::UnsubscribePeer(peer)));
                } else {
                    self.queue
                        .push_back(InternalOutput::RpcRes(req_id, EndpointRes::UnsubscribePeer(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))));
                }
            }
            EndpointReq::RemoteTrack(track_id, req) => {
                let index = return_if_none!(self.remote_tracks_id.get1(&track_id));
                self.remote_tracks.input(&mut self.switcher).on_event(now, *index, remote_track::Input::RpcReq(req_id, req));
            }
            EndpointReq::LocalTrack(track_id, req) => {
                let index = return_if_none!(self.local_tracks_id.get1(&track_id));
                self.local_tracks.input(&mut self.switcher).on_event(now, *index, local_track::Input::RpcReq(req_id, req));
            }
        }
    }

    fn on_transport_state_changed<'a>(&mut self, now: Instant, state: TransportState) {
        self.state = state;
        match &self.state {
            TransportState::Connecting => {
                log::info!("[EndpointInternal] connecting");
            }
            TransportState::ConnectError(err) => {
                log::info!("[EndpointInternal] connect error {:?}", err);
                self.queue.push_back(InternalOutput::Destroy);
            }
            TransportState::Connected => {
                log::info!("[EndpointInternal] connected");
                let (req_id, room, peer, meta, publish, subscribe) = return_if_none!(self.wait_join.take());
                log::info!("[EndpointInternal] join_room({room}, {peer}) after connected");
                self.join_room(now, req_id, room, peer, meta, publish, subscribe);
            }
            TransportState::Reconnecting => {
                log::info!("[EndpointInternal] reconnecting");
            }
            TransportState::Disconnected(err) => {
                log::info!("[EndpointInternal] disconnected {:?}", err);
                self.leave_room(now);
                self.queue.push_back(InternalOutput::Destroy);
            }
        }
    }

    fn on_transport_remote_track<'a>(&mut self, now: Instant, track: RemoteTrackId, event: RemoteTrackEvent) {
        if let Some(meta) = event.need_create() {
            log::info!("[EndpointInternal] create remote track {:?}", track);
            let room = self.joined.as_ref().map(|j| j.0.clone());
            let index = self.remote_tracks.input(&mut self.switcher).add_task(EndpointRemoteTrack::new(room, meta));
            self.remote_tracks_id.insert(track, index);
        }
        let index = return_if_none!(self.remote_tracks_id.get1(&track));
        self.remote_tracks.input(&mut self.switcher).on_event(now, *index, remote_track::Input::Event(event));
    }

    fn on_transport_local_track<'a>(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) {
        if let Some(kind) = event.need_create() {
            log::info!("[EndpointInternal] create local track {:?}", track);
            let room = self.joined.as_ref().map(|j| j.0.clone());
            let index = self.local_tracks.input(&mut self.switcher).add_task(EndpointLocalTrack::new(kind, room));
            self.local_tracks_id.insert(track, index);
        }
        let index = return_if_none!(self.local_tracks_id.get1(&track));
        self.local_tracks.input(&mut self.switcher).on_event(now, *index, local_track::Input::Event(event));
    }

    fn on_transport_stats<'a>(&mut self, _now: Instant, _stats: TransportStats) {}

    fn join_room<'a>(&mut self, now: Instant, req_id: EndpointReqId, room: RoomId, peer: PeerId, meta: PeerMeta, publish: RoomInfoPublish, subscribe: RoomInfoSubscribe) {
        let room_hash: ClusterRoomHash = (&room).into();
        log::info!("[EndpointInternal] join_room({room}, {peer}), room_hash {room_hash}");
        self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::JoinRoom(Ok(()))));

        self.leave_room(now);

        self.joined = Some(((&room).into(), room.clone(), peer.clone()));
        self.queue
            .push_back(InternalOutput::Cluster((&room).into(), ClusterEndpointControl::Join(peer, meta, publish, subscribe)));

        for (_track_id, index) in self.local_tracks_id.pairs() {
            self.local_tracks.input(&mut self.switcher).on_event(now, index, local_track::Input::JoinRoom(room_hash));
        }

        for (_track_id, index) in self.remote_tracks_id.pairs() {
            self.remote_tracks.input(&mut self.switcher).on_event(now, index, remote_track::Input::JoinRoom(room_hash));
        }
    }

    fn leave_room<'a>(&mut self, now: Instant) {
        let (hash, room, peer) = return_if_none!(self.joined.take());
        log::info!("[EndpointInternal] leave_room({room}, {peer})");

        for (_track_id, index) in self.local_tracks_id.pairs() {
            self.local_tracks.input(&mut self.switcher).on_event(now, index, local_track::Input::LeaveRoom);
        }

        for (_track_id, index) in self.remote_tracks_id.pairs() {
            self.remote_tracks.input(&mut self.switcher).on_event(now, index, remote_track::Input::LeaveRoom);
        }

        self.queue.push_back(InternalOutput::Cluster(hash, ClusterEndpointControl::Leave));
    }
}

/// This block is for cluster related events
impl EndpointInternal {
    pub fn on_cluster_event<'a>(&mut self, now: Instant, event: ClusterEndpointEvent) {
        match event {
            ClusterEndpointEvent::PeerJoined(peer, meta) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerJoined(peer, meta))),
            ClusterEndpointEvent::PeerLeaved(peer) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerLeaved(peer))),
            ClusterEndpointEvent::TrackStarted(peer, track, meta) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerTrackStarted(peer, track, meta))),
            ClusterEndpointEvent::TrackStopped(peer, track) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerTrackStopped(peer, track))),
            ClusterEndpointEvent::RemoteTrack(track, event) => self.on_cluster_remote_track(now, track, event),
            ClusterEndpointEvent::LocalTrack(track, event) => self.on_cluster_local_track(now, track, event),
        }
    }

    fn on_cluster_remote_track<'a>(&mut self, now: Instant, id: RemoteTrackId, event: ClusterRemoteTrackEvent) {
        let index = return_if_none!(self.remote_tracks_id.get1(&id));
        self.remote_tracks.input(&mut self.switcher).on_event(now, *index, remote_track::Input::Cluster(event));
    }

    fn on_cluster_local_track<'a>(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) {
        let index = return_if_none!(self.local_tracks_id.get1(&id));
        self.local_tracks.input(&mut self.switcher).on_event(now, *index, local_track::Input::Cluster(event));
    }
}

/// This block for internal local and remote track
impl EndpointInternal {
    fn pop_remote_tracks<'a>(&mut self, now: Instant) {
        let (index, out) = return_if_none!(self.remote_tracks.pop_output(now, &mut self.switcher));
        let id = *self.remote_tracks_id.get2(&index).expect("Should have remote_track_id");

        match out {
            remote_track::Output::Event(event) => {
                self.queue.push_back(InternalOutput::Event(EndpointEvent::RemoteMediaTrack(id, event)));
            }
            remote_track::Output::Cluster(room, control) => {
                self.queue.push_back(InternalOutput::Cluster(room, ClusterEndpointControl::RemoteTrack(id, control)));
            }
            remote_track::Output::RpcRes(req_id, res) => {
                self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::RemoteTrack(id, res)));
            }
            remote_track::Output::Started(kind, priority) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).set_ingress_video_track(id, priority);
                }
            }
            remote_track::Output::Stopped(kind) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).del_ingress_video_track(id);
                }
            }
        }
    }

    fn pop_local_tracks(&mut self, now: Instant) {
        let (index, out) = return_if_none!(self.local_tracks.pop_output(now, &mut self.switcher));
        let id = *self.local_tracks_id.get2(&index).expect("Should have local_track_id");
        match out {
            local_track::Output::Event(event) => {
                self.queue.push_back(InternalOutput::Event(EndpointEvent::LocalMediaTrack(id, event)));
            }
            local_track::Output::Cluster(room, control) => {
                self.queue.push_back(InternalOutput::Cluster(room, ClusterEndpointControl::LocalTrack(id, control)));
            }
            local_track::Output::RpcRes(req_id, res) => {
                self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::LocalTrack(id, res)));
            }
            local_track::Output::Started(kind, priority) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).set_egress_video_track(id, priority);
                }
            }
            local_track::Output::Updated(kind, priority) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).set_egress_video_track(id, priority);
                }
            }
            local_track::Output::Stopped(kind) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).del_egress_video_track(id);
                }
            }
        }
    }

    fn pop_bitrate_allocator(&mut self, now: Instant) {
        if let Some(out) = self.bitrate_allocator.pop_output(now, &mut self.switcher) {
            match out {
                bitrate_allocator::Output::RemoteTrack(track, action) => {
                    if let Some(index) = self.remote_tracks_id.get1(&track) {
                        self.remote_tracks.input(&mut self.switcher).on_event(now, *index, remote_track::Input::BitrateAllocation(action));
                    }
                }
                bitrate_allocator::Output::LocalTrack(track, action) => {
                    if let Some(index) = self.local_tracks_id.get1(&track) {
                        self.local_tracks.input(&mut self.switcher).on_event(now, *index, local_track::Input::BitrateAllocation(action));
                    }
                }
                bitrate_allocator::Output::BweConfig(current, desired) => {
                    self.queue.push_back(InternalOutput::Event(EndpointEvent::BweConfig { current, desired }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use media_server_protocol::endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::{
        cluster::{ClusterEndpointControl, ClusterRoomHash},
        endpoint::{internal::InternalOutput, EndpointCfg, EndpointReq, EndpointRes},
        transport::{TransportEvent, TransportState},
    };

    use super::EndpointInternal;

    #[test]
    fn test_join_leave_room_success() {
        let mut internal = EndpointInternal::new(EndpointCfg {
            max_egress_bitrate: 2_000_000,
            max_ingress_bitrate: 2_000_000,
        });

        let now = Instant::now();
        internal.on_transport_event(now, TransportEvent::State(TransportState::Connected));
        assert_eq!(internal.pop_output(now), None);

        let room: RoomId = "room".into();
        let peer: PeerId = "peer".into();
        let meta = PeerMeta { metadata: None };
        let publish = RoomInfoPublish { peer: true, tracks: true };
        let subscribe = RoomInfoSubscribe { peers: true, tracks: true };
        internal.on_transport_rpc(now, 0.into(), EndpointReq::JoinRoom(room.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone()));
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(0.into(), EndpointRes::JoinRoom(Ok(())))));
        let room_hash = ClusterRoomHash::from(&room);
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(room_hash, ClusterEndpointControl::Join(peer, meta, publish, subscribe)))
        );
        assert_eq!(internal.pop_output(now), None);

        //now leave room should success
        internal.on_transport_rpc(now, 1.into(), EndpointReq::LeaveRoom);
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(1.into(), EndpointRes::LeaveRoom(Ok(())))));
        assert_eq!(internal.pop_output(now), Some(InternalOutput::Cluster(room_hash, ClusterEndpointControl::Leave)));
        assert_eq!(internal.pop_output(now), None);
    }

    #[test]
    fn test_join_overwrite_auto_leave() {
        let mut internal = EndpointInternal::new(EndpointCfg {
            max_egress_bitrate: 2_000_000,
            max_ingress_bitrate: 2_000_000,
        });

        let now = Instant::now();
        internal.on_transport_event(now, TransportEvent::State(TransportState::Connected));
        assert_eq!(internal.pop_output(now), None);

        let room1: RoomId = "room1".into();
        let room1_hash = ClusterRoomHash::from(&room1);
        let peer: PeerId = "peer".into();
        let meta = PeerMeta { metadata: None };
        let publish = RoomInfoPublish { peer: true, tracks: true };
        let subscribe = RoomInfoSubscribe { peers: true, tracks: true };
        internal.on_transport_rpc(now, 0.into(), EndpointReq::JoinRoom(room1.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone()));
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(0.into(), EndpointRes::JoinRoom(Ok(())))));

        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(
                room1_hash,
                ClusterEndpointControl::Join(peer.clone(), meta.clone(), publish.clone(), subscribe.clone())
            ))
        );
        assert_eq!(internal.pop_output(now), None);

        //now join other room should success
        let room2: RoomId = "room2".into();
        let room2_hash = ClusterRoomHash::from(&room2);

        internal.on_transport_rpc(now, 1.into(), EndpointReq::JoinRoom(room2.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone()));
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(1.into(), EndpointRes::JoinRoom(Ok(())))));
        //it will auto leave room1
        assert_eq!(internal.pop_output(now), Some(InternalOutput::Cluster(room1_hash, ClusterEndpointControl::Leave)));

        //and after that join room2
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(
                room2_hash,
                ClusterEndpointControl::Join(peer.clone(), meta.clone(), publish.clone(), subscribe.clone())
            ))
        );
        assert_eq!(internal.pop_output(now), None);
    }
    //TODO single local track, join leave room
    //TODO multi local tracks, join leave room
    //TODO single remote track, join leave room
    //TODO multi remote tracks, join leave room
    //TODO both local and remote tracks, join leave room
    //TODO handle close request
    //TODO handle transport connected
    //TODO handle transport disconnected
}
