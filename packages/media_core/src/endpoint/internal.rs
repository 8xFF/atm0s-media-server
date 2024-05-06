//! EndpointInternal compose small parts: local track, remote track. It act as integration hub

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe},
    transport::RpcError,
};
use media_server_utils::Small2dMap;
use sans_io_runtime::{TaskGroup, TaskSwitcher};

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

#[derive(num_enum::TryFromPrimitive)]
#[repr(usize)]
enum TaskType {
    LocalTracks,
    RemoteTracks,
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
    local_tracks: TaskGroup<local_track::Input, local_track::Output, EndpointLocalTrack, 4>,
    remote_tracks: TaskGroup<remote_track::Input, remote_track::Output, EndpointRemoteTrack, 16>,
    _middlewares: Vec<Box<dyn EndpointMiddleware>>,
    queue: VecDeque<InternalOutput>,
    switcher: TaskSwitcher,
    bitrate_allocator: BitrateAllocator,
}

impl EndpointInternal {
    pub fn new(cfg: EndpointCfg) -> Self {
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
            bitrate_allocator: BitrateAllocator::new(cfg.max_ingress_bitrate, cfg.max_ingress_bitrate),
            cfg,
        }
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        self.bitrate_allocator.on_tick();
        if let Some(out) = self.bitrate_allocator.pop_output() {
            match out {
                bitrate_allocator::Output::RemoteTrack(track, action) => {
                    if let Some(index) = self.remote_tracks_id.get1(&track) {
                        let out = self.remote_tracks.on_event(now, *index, remote_track::Input::BitrateAllocation(action))?;
                        self.convert_remote_track_output(now, track, out);
                        if let Some(out) = self.queue.pop_front() {
                            return Some(out);
                        }
                    }
                }
                bitrate_allocator::Output::LocalTrack(track, action) => {
                    if let Some(index) = self.local_tracks_id.get1(&track) {
                        let out = self.local_tracks.on_event(now, *index, local_track::Input::BitrateAllocation(action))?;
                        self.convert_local_track_output(now, track, out);
                        if let Some(out) = self.queue.pop_front() {
                            return Some(out);
                        }
                    }
                }
                bitrate_allocator::Output::BweConfig(current, desired) => {
                    return Some(InternalOutput::Event(EndpointEvent::BweConfig { current, desired }));
                }
            }
        }

        loop {
            match self.switcher.looper_current(now)?.try_into().ok()? {
                TaskType::LocalTracks => {
                    if let Some((index, out)) = self.switcher.looper_process(self.local_tracks.on_tick(now)) {
                        let track_id = self.local_tracks_id.get2(&index).expect("Should have local_track_id");
                        self.convert_local_track_output(now, *track_id, out);
                        if let Some(out) = self.queue.pop_front() {
                            return Some(out);
                        }
                    }
                }
                TaskType::RemoteTracks => {
                    if let Some((index, out)) = self.switcher.looper_process(self.remote_tracks.on_tick(now)) {
                        let track_id = self.remote_tracks_id.get2(&index).expect("Should have remote_track_id");
                        self.convert_remote_track_output(now, *track_id, out);
                        if let Some(out) = self.queue.pop_front() {
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
                        self.convert_local_track_output(now, *track_id, out);
                        if let Some(out) = self.queue.pop_front() {
                            return Some(out);
                        }
                    }
                }
                TaskType::RemoteTracks => {
                    if let Some((index, out)) = self.switcher.queue_process(self.remote_tracks.pop_output(now)) {
                        let track_id = self.remote_tracks_id.get2(&index).expect("Should have remote_track_id");
                        self.convert_remote_track_output(now, *track_id, out);
                        if let Some(out) = self.queue.pop_front() {
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
            TransportEvent::EgressBitrateEstimate(bitrate) => {
                let bitrate2 = bitrate.min(self.cfg.max_egress_bitrate as u64);
                log::debug!("[EndpointInternal] limit egress bitrate {bitrate2}, rewrite from {bitrate}");
                self.bitrate_allocator.set_egress_estimate(bitrate2);
                None
            }
        }
    }

    pub fn on_transport_rpc<'a>(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointReq) -> Option<InternalOutput> {
        match req {
            EndpointReq::JoinRoom(room, peer, meta, publish, subscribe) => {
                if matches!(self.state, TransportState::Connecting) {
                    log::info!("[EndpointInternal] join_room({room}, {peer}) but in Connecting state => wait");
                    self.wait_join = Some((req_id, room, peer, meta, publish, subscribe));
                    None
                } else {
                    self.join_room(now, req_id, room, peer, meta, publish, subscribe)
                }
            }
            EndpointReq::LeaveRoom => {
                if let Some((_req_id, room, peer, _meta, _publish, _subscribe)) = self.wait_join.take() {
                    log::info!("[EndpointInternal] leave_room({room}, {peer}) but in Connecting state => only clear local");
                    Some(InternalOutput::RpcRes(req_id, EndpointRes::LeaveRoom(Ok(()))))
                } else {
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::LeaveRoom(Ok(()))));
                    self.leave_room(now)
                }
            }
            EndpointReq::SubscribePeer(peer) => {
                if let Some((room, _, _)) = &self.joined {
                    self.queue.push_back(InternalOutput::Cluster(*room, ClusterEndpointControl::SubscribePeer(peer)));
                    Some(InternalOutput::RpcRes(req_id, EndpointRes::SubscribePeer(Ok(()))))
                } else {
                    Some(InternalOutput::RpcRes(req_id, EndpointRes::SubscribePeer(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))))
                }
            }
            EndpointReq::UnsubscribePeer(peer) => {
                if let Some((room, _, _)) = &self.joined {
                    self.queue.push_back(InternalOutput::Cluster(*room, ClusterEndpointControl::UnsubscribePeer(peer)));
                    Some(InternalOutput::RpcRes(req_id, EndpointRes::UnsubscribePeer(Ok(()))))
                } else {
                    Some(InternalOutput::RpcRes(req_id, EndpointRes::UnsubscribePeer(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))))
                }
            }
            EndpointReq::RemoteTrack(track_id, req) => {
                let index = self.remote_tracks_id.get1(&track_id)?;
                let out = self.remote_tracks.on_event(now, *index, remote_track::Input::RpcReq(req_id, req))?;
                self.convert_remote_track_output(now, track_id, out);
                self.queue.pop_front()
            }
            EndpointReq::LocalTrack(track_id, req) => {
                let index = self.local_tracks_id.get1(&track_id)?;
                let out = self.local_tracks.on_event(now, *index, local_track::Input::RpcReq(req_id, req))?;
                self.convert_local_track_output(now, track_id, out);
                self.queue.pop_front()
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
                let (req_id, room, peer, meta, publish, subscribe) = self.wait_join.take()?;
                log::info!("[EndpointInternal] join_room({room}, {peer}) after connected");
                self.join_room(now, req_id, room, peer, meta, publish, subscribe)
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
        self.convert_remote_track_output(now, track, out);
        self.queue.pop_front()
    }

    fn on_transport_local_track<'a>(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) -> Option<InternalOutput> {
        if let Some(kind) = event.need_create() {
            log::info!("[EndpointInternal] create local track {:?}", track);
            let room = self.joined.as_ref().map(|j| j.0.clone());
            let index = self.local_tracks.add_task(EndpointLocalTrack::new(kind, room));
            self.local_tracks_id.insert(track, index);
        }
        let index = self.local_tracks_id.get1(&track)?;
        let out = self.local_tracks.on_event(now, *index, local_track::Input::Event(event))?;
        self.convert_local_track_output(now, track, out);
        self.queue.pop_front()
    }

    fn on_transport_stats<'a>(&mut self, _now: Instant, _stats: TransportStats) -> Option<InternalOutput> {
        None
    }

    fn join_room<'a>(&mut self, now: Instant, req_id: EndpointReqId, room: RoomId, peer: PeerId, meta: PeerMeta, publish: RoomInfoPublish, subscribe: RoomInfoSubscribe) -> Option<InternalOutput> {
        let room_hash: ClusterRoomHash = (&room).into();
        log::info!("[EndpointInternal] join_room({room}, {peer}), room_hash {room_hash}");
        self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::JoinRoom(Ok(()))));

        if let Some(out) = self.leave_room(now) {
            self.queue.push_front(out);
        }

        self.joined = Some(((&room).into(), room.clone(), peer.clone()));
        self.queue
            .push_back(InternalOutput::Cluster((&room).into(), ClusterEndpointControl::Join(peer, meta, publish, subscribe)));

        for (track_id, index) in self.local_tracks_id.pairs() {
            if let Some(out) = self.local_tracks.on_event(now, index, local_track::Input::JoinRoom(room_hash)) {
                self.convert_local_track_output(now, track_id, out);
            }
        }

        for (track_id, index) in self.remote_tracks_id.pairs() {
            if let Some(out) = self.remote_tracks.on_event(now, index, remote_track::Input::JoinRoom(room_hash)) {
                self.convert_remote_track_output(now, track_id, out);
            }
        }

        let out = self.queue.pop_front();

        log::info!("after pop {:?} queue size {}", out, self.queue.len());
        out
    }

    fn leave_room<'a>(&mut self, now: Instant) -> Option<InternalOutput> {
        let (hash, room, peer) = self.joined.take()?;
        log::info!("[EndpointInternal] leave_room({room}, {peer})");

        for (track_id, index) in self.local_tracks_id.pairs() {
            if let Some(out) = self.local_tracks.on_event(now, index, local_track::Input::LeaveRoom) {
                self.convert_local_track_output(now, track_id, out);
            }
        }

        for (track_id, index) in self.remote_tracks_id.pairs() {
            if let Some(out) = self.remote_tracks.on_event(now, index, remote_track::Input::LeaveRoom) {
                self.convert_remote_track_output(now, track_id, out);
            }
        }

        self.queue.push_back(InternalOutput::Cluster(hash, ClusterEndpointControl::Leave));
        self.queue.pop_front()
    }
}

/// This block is for cluster related events
impl EndpointInternal {
    pub fn on_cluster_event<'a>(&mut self, now: Instant, event: ClusterEndpointEvent) -> Option<InternalOutput> {
        match event {
            ClusterEndpointEvent::PeerJoined(peer, meta) => Some(InternalOutput::Event(EndpointEvent::PeerJoined(peer, meta))),
            ClusterEndpointEvent::PeerLeaved(peer) => Some(InternalOutput::Event(EndpointEvent::PeerLeaved(peer))),
            ClusterEndpointEvent::TrackStarted(peer, track, meta) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStarted(peer, track, meta))),
            ClusterEndpointEvent::TrackStopped(peer, track) => Some(InternalOutput::Event(EndpointEvent::PeerTrackStopped(peer, track))),
            ClusterEndpointEvent::RemoteTrack(track, event) => self.on_cluster_remote_track(now, track, event),
            ClusterEndpointEvent::LocalTrack(track, event) => self.on_cluster_local_track(now, track, event),
        }
    }

    fn on_cluster_remote_track<'a>(&mut self, now: Instant, id: RemoteTrackId, event: ClusterRemoteTrackEvent) -> Option<InternalOutput> {
        let index = self.remote_tracks_id.get1(&id)?;
        let out = self.remote_tracks.on_event(now, *index, remote_track::Input::Cluster(event))?;
        self.convert_remote_track_output(now, id, out);
        self.queue.pop_front()
    }

    fn on_cluster_local_track<'a>(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) -> Option<InternalOutput> {
        let index = self.local_tracks_id.get1(&id)?;
        let out = self.local_tracks.on_event(now, *index, local_track::Input::Cluster(event))?;
        self.convert_local_track_output(now, id, out);
        self.queue.pop_front()
    }
}

/// This block for internal local and remote track
impl EndpointInternal {
    fn convert_remote_track_output<'a>(&mut self, _now: Instant, id: RemoteTrackId, out: remote_track::Output) {
        self.switcher.queue_flag_task(TaskType::RemoteTracks as usize);
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
                    self.bitrate_allocator.set_ingress_video_track(id, priority);
                }
            }
            remote_track::Output::Stopped(kind) => {
                if kind.is_video() {
                    self.bitrate_allocator.del_ingress_video_track(id);
                }
            }
        }
    }

    fn convert_local_track_output<'a>(&mut self, _now: Instant, id: LocalTrackId, out: local_track::Output) {
        self.switcher.queue_flag_task(TaskType::LocalTracks as usize);
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
                    self.bitrate_allocator.set_egress_video_track(id, priority);
                }
            }
            local_track::Output::Stopped(kind) => {
                if kind.is_video() {
                    self.bitrate_allocator.del_egress_video_track(id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use media_server_protocol::endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe};

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
        let out = internal.on_transport_event(now, TransportEvent::State(TransportState::Connected));
        assert_eq!(out, None);

        let room: RoomId = "room".into();
        let peer: PeerId = "peer".into();
        let meta = PeerMeta { metadata: None };
        let publish = RoomInfoPublish { peer: true, tracks: true };
        let subscribe = RoomInfoSubscribe { peers: true, tracks: true };
        let out = internal.on_transport_rpc(now, 0.into(), EndpointReq::JoinRoom(room.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone()));
        assert_eq!(out, Some(InternalOutput::RpcRes(0.into(), EndpointRes::JoinRoom(Ok(())))));
        let room_hash = ClusterRoomHash::from(&room);
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(room_hash, ClusterEndpointControl::Join(peer, meta, publish, subscribe)))
        );
        assert_eq!(internal.pop_output(now), None);

        //now leave room should success
        let out = internal.on_transport_rpc(now, 1.into(), EndpointReq::LeaveRoom);
        assert_eq!(out, Some(InternalOutput::RpcRes(1.into(), EndpointRes::LeaveRoom(Ok(())))));
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
        let out = internal.on_transport_event(now, TransportEvent::State(TransportState::Connected));
        assert_eq!(out, None);

        let room1: RoomId = "room1".into();
        let room1_hash = ClusterRoomHash::from(&room1);
        let peer: PeerId = "peer".into();
        let meta = PeerMeta { metadata: None };
        let publish = RoomInfoPublish { peer: true, tracks: true };
        let subscribe = RoomInfoSubscribe { peers: true, tracks: true };
        let out = internal.on_transport_rpc(now, 0.into(), EndpointReq::JoinRoom(room1.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone()));
        assert_eq!(out, Some(InternalOutput::RpcRes(0.into(), EndpointRes::JoinRoom(Ok(())))));

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

        let out = internal.on_transport_rpc(now, 1.into(), EndpointReq::JoinRoom(room2.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone()));
        assert_eq!(out, Some(InternalOutput::RpcRes(1.into(), EndpointRes::JoinRoom(Ok(())))));
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
