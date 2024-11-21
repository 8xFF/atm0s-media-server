//! EndpointInternal compose small parts: local track, remote track. It act as integration hub

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{AudioMixerConfig, AudioMixerMode, PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe},
    protobuf::{cluster_connector::peer_event, shared::Kind},
    record::SessionRecordEvent,
    transport::RpcError,
};
use media_server_utils::IndexMap2d;
use sans_io_runtime::{return_if_none, return_if_some, TaskGroup, TaskGroupOutput, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::{
    cluster::{
        ClusterAudioMixerControl, ClusterAudioMixerEvent, ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterMessageChannelControl, ClusterRemoteTrackEvent, ClusterRoomHash,
    },
    errors::EndpointErrors,
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportEvent, TransportState, TransportStats},
};

use self::{bitrate_allocator::BitrateAllocator, local_track::EndpointLocalTrack, remote_track::EndpointRemoteTrack};

use super::{
    EndpointAudioMixerEvent, EndpointAudioMixerReq, EndpointAudioMixerRes, EndpointCfg, EndpointEvent, EndpointMessageChannelReq, EndpointMessageChannelRes, EndpointReq, EndpointReqId, EndpointRes,
};

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

#[derive(Debug, PartialEq)]
pub enum InternalOutput {
    Event(EndpointEvent),
    PeerEvent(Instant, peer_event::Event),
    RecordEvent(Instant, SessionRecordEvent),
    RpcRes(EndpointReqId, EndpointRes),
    Cluster(ClusterRoomHash, ClusterEndpointControl),
    OnResourceEmpty,
}

type EndpointInternalWaitJoin = Option<(EndpointReqId, RoomId, PeerId, PeerMeta, RoomInfoPublish, RoomInfoSubscribe, Option<AudioMixerConfig>)>;

pub struct EndpointInternal {
    cfg: EndpointCfg,
    state: Option<(Instant, TransportState)>,
    wait_join: EndpointInternalWaitJoin,
    joined: Option<(ClusterRoomHash, RoomId, PeerId, Option<AudioMixerMode>)>,
    local_tracks_id: IndexMap2d<LocalTrackId, usize>,
    remote_tracks_id: IndexMap2d<RemoteTrackId, usize>,
    local_tracks: TaskSwitcherBranch<TaskGroup<local_track::Input, local_track::Output, EndpointLocalTrack, 4>, TaskGroupOutput<local_track::Output>>,
    remote_tracks: TaskSwitcherBranch<TaskGroup<remote_track::Input, remote_track::Output, EndpointRemoteTrack, 16>, TaskGroupOutput<remote_track::Output>>,
    bitrate_allocator: TaskSwitcherBranch<BitrateAllocator, bitrate_allocator::Output>,
    queue: VecDeque<InternalOutput>,
    shutdown: bool,
    switcher: TaskSwitcher,
}

impl EndpointInternal {
    pub fn new(cfg: EndpointCfg) -> Self {
        Self {
            state: None,
            wait_join: None,
            joined: None,
            local_tracks_id: Default::default(),
            remote_tracks_id: Default::default(),
            local_tracks: TaskSwitcherBranch::default(TaskType::LocalTracks),
            remote_tracks: TaskSwitcherBranch::default(TaskType::RemoteTracks),
            bitrate_allocator: TaskSwitcherBranch::new(BitrateAllocator::new(cfg.max_ingress_bitrate, cfg.max_ingress_bitrate), TaskType::BitrateAllocator),
            queue: Default::default(),
            shutdown: false,
            switcher: TaskSwitcher::new(3),
            cfg,
        }
    }

    pub fn on_tick(&mut self, now: Instant) {
        self.bitrate_allocator.input(&mut self.switcher).on_tick();
        self.local_tracks.input(&mut self.switcher).on_tick(now);
        self.remote_tracks.input(&mut self.switcher).on_tick(now);
    }

    pub fn on_shutdown(&mut self, now: Instant) {
        if self.shutdown {
            return;
        }
        self.shutdown = true;
        self.local_tracks.input(&mut self.switcher).on_shutdown(now);
        self.remote_tracks.input(&mut self.switcher).on_shutdown(now);

        // after shutdown, we need to pop all the remaining tasks
        while let Some(task) = self.switcher.current() {
            match task.try_into().expect("Should valid task type") {
                TaskType::BitrateAllocator => self.pop_bitrate_allocator(now),
                TaskType::LocalTracks => self.pop_local_tracks(now),
                TaskType::RemoteTracks => self.pop_remote_tracks(now),
            }
        }

        // if joined, send leave event
        let (hash, room, peer, _) = return_if_none!(self.joined.take());
        self.queue.push_back(InternalOutput::Cluster(hash, ClusterEndpointControl::Leave));
        if self.cfg.record {
            self.queue.push_back(InternalOutput::RecordEvent(now, SessionRecordEvent::LeaveRoom));
        }
        self.queue
            .push_back(InternalOutput::PeerEvent(now, peer_event::Event::Leave(peer_event::Leave { room: room.into(), peer: peer.into() })));
    }
}

impl TaskSwitcherChild<InternalOutput> for EndpointInternal {
    type Time = Instant;

    fn is_empty(&self) -> bool {
        self.shutdown && self.queue.is_empty() && self.local_tracks.is_empty() && self.remote_tracks.is_empty()
    }

    fn empty_event(&self) -> InternalOutput {
        InternalOutput::OnResourceEmpty
    }

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
                let bitrate2 = bitrate.min(self.cfg.max_egress_bitrate);
                log::debug!("[EndpointInternal] limit egress bitrate {bitrate2}, rewrite from {bitrate}");
                self.bitrate_allocator.input(&mut self.switcher).set_egress_estimate(bitrate2);
            }
        }
    }

    pub fn on_transport_rpc(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointReq) {
        match req {
            EndpointReq::JoinRoom(room, peer, meta, publish, subscribe, mixer) => match &self.state {
                None | Some((_, TransportState::Connecting(_))) => {
                    log::info!("[EndpointInternal] join_room({room}, {peer}) but in Connecting state => wait");
                    self.wait_join = Some((req_id, room, peer, meta, publish, subscribe, mixer));
                }
                _ => {
                    self.join_room(now, req_id, room, peer, meta, publish, subscribe, mixer);
                }
            },
            EndpointReq::LeaveRoom => {
                if let Some((_req_id, room, peer, _meta, _publish, _subscribe, _mixer)) = self.wait_join.take() {
                    log::info!("[EndpointInternal] leave_room({room}, {peer}) but in Connecting state => only clear local");
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::LeaveRoom(Ok(()))));
                } else {
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::LeaveRoom(Ok(()))));
                    self.leave_room(now);
                }
            }
            EndpointReq::SubscribePeer(peer) => {
                if let Some((room, _, _, _)) = &self.joined {
                    self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::SubscribePeer(Ok(()))));
                    self.queue.push_back(InternalOutput::Cluster(*room, ClusterEndpointControl::SubscribePeer(peer)));
                } else {
                    self.queue
                        .push_back(InternalOutput::RpcRes(req_id, EndpointRes::SubscribePeer(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))));
                }
            }
            EndpointReq::UnsubscribePeer(peer) => {
                if let Some((room, _, _, _)) = &self.joined {
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
            EndpointReq::AudioMixer(req) => match req {
                EndpointAudioMixerReq::Attach(sources) => {
                    if let Some((room, _, _, Some(AudioMixerMode::Manual))) = &self.joined {
                        self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::AudioMixer(EndpointAudioMixerRes::Attach(Ok(())))));
                        self.queue
                            .push_back(InternalOutput::Cluster(*room, ClusterEndpointControl::AudioMixer(ClusterAudioMixerControl::Attach(sources))));
                    } else {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::AudioMixer(EndpointAudioMixerRes::Attach(Err(RpcError::new2(EndpointErrors::AudioMixerWrongMode)))),
                        ));
                    }
                }
                EndpointAudioMixerReq::Detach(sources) => {
                    if let Some((room, _, _, Some(AudioMixerMode::Manual))) = &self.joined {
                        self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::AudioMixer(EndpointAudioMixerRes::Detach(Ok(())))));
                        self.queue
                            .push_back(InternalOutput::Cluster(*room, ClusterEndpointControl::AudioMixer(ClusterAudioMixerControl::Detach(sources))));
                    } else {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::AudioMixer(EndpointAudioMixerRes::Detach(Err(RpcError::new2(EndpointErrors::AudioMixerWrongMode)))),
                        ));
                    }
                }
            },
            EndpointReq::MessageChannel(label, control) => match control {
                EndpointMessageChannelReq::Subscribe => {
                    if let Some((room, _, _, _)) = &self.joined {
                        self.queue
                            .push_back(InternalOutput::RpcRes(req_id, EndpointRes::MessageChannel(label.clone(), EndpointMessageChannelRes::Subscribe(Ok(())))));
                        self.queue.push_back(InternalOutput::Cluster(
                            *room,
                            ClusterEndpointControl::MessageChannel(label.clone(), ClusterMessageChannelControl::Subscribe),
                        ));
                    } else {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label, EndpointMessageChannelRes::Subscribe(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))),
                        ));
                    }
                }
                EndpointMessageChannelReq::Unsubscribe => {
                    if let Some((room, _, _, _)) = &self.joined {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label.clone(), EndpointMessageChannelRes::Unsubscribe(Ok(()))),
                        ));
                        self.queue.push_back(InternalOutput::Cluster(
                            *room,
                            ClusterEndpointControl::MessageChannel(label.clone(), ClusterMessageChannelControl::Unsubscribe),
                        ));
                    } else {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label, EndpointMessageChannelRes::Unsubscribe(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))),
                        ));
                    }
                }
                EndpointMessageChannelReq::StartPublish => {
                    if let Some((room, _, _, _)) = &self.joined {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label.clone(), EndpointMessageChannelRes::StartPublish(Ok(()))),
                        ));
                        self.queue.push_back(InternalOutput::Cluster(
                            *room,
                            ClusterEndpointControl::MessageChannel(label.clone(), ClusterMessageChannelControl::StartPublish),
                        ));
                    } else {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label, EndpointMessageChannelRes::StartPublish(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))),
                        ));
                    }
                }
                EndpointMessageChannelReq::StopPublish => {
                    if let Some((room, _, _, _)) = &self.joined {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label.clone(), EndpointMessageChannelRes::StopPublish(Ok(()))),
                        ));
                        self.queue.push_back(InternalOutput::Cluster(
                            *room,
                            ClusterEndpointControl::MessageChannel(label.clone(), ClusterMessageChannelControl::StopPublish),
                        ));
                    } else {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label, EndpointMessageChannelRes::StopPublish(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))),
                        ));
                    }
                }
                EndpointMessageChannelReq::PublishData(data) => {
                    if let Some((room, _, peer_id, _)) = &self.joined {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label.clone(), EndpointMessageChannelRes::PublishData(Ok(()))),
                        ));
                        self.queue.push_back(InternalOutput::Cluster(
                            *room,
                            ClusterEndpointControl::MessageChannel(label.clone(), ClusterMessageChannelControl::PublishData(peer_id.clone(), data)),
                        ));
                    } else {
                        self.queue.push_back(InternalOutput::RpcRes(
                            req_id,
                            EndpointRes::MessageChannel(label, EndpointMessageChannelRes::PublishData(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))),
                        ));
                    }
                }
            },
        }
    }

    fn on_transport_state_changed(&mut self, now: Instant, state: TransportState) {
        let pre_state = self.state.take();
        self.state = Some((now, state));
        match &(self.state.as_ref().expect("Should have state").1) {
            TransportState::New => {
                log::info!("[EndpointInternal] new state");
            }
            TransportState::Connecting(ip) => {
                log::info!("[EndpointInternal] connecting");
                self.queue
                    .push_back(InternalOutput::PeerEvent(now, peer_event::Event::Connecting(peer_event::Connecting { remote_ip: ip.to_string() })));
            }
            TransportState::ConnectError(err) => {
                log::info!("[EndpointInternal] connect error {:?}", err);
                let (pre_ts, _pre_event) = pre_state.expect("Should have previous state");
                self.queue.push_back(InternalOutput::PeerEvent(
                    now,
                    peer_event::Event::ConnectError(peer_event::ConnectError {
                        after_ms: (pre_ts - now).as_millis() as u32,
                        error: 0,
                    }),
                ));
                self.on_shutdown(now);
            }
            TransportState::Connected(ip) => {
                log::info!("[EndpointInternal] connected");
                let (pre_ts, pre_event) = pre_state.expect("Should have previous state");
                if matches!(pre_event, TransportState::Reconnecting(_)) {
                    self.queue.push_back(InternalOutput::PeerEvent(
                        now,
                        peer_event::Event::Reconnected(peer_event::Reconnected {
                            after_ms: (pre_ts - now).as_millis() as u32,
                            remote_ip: ip.to_string(),
                        }),
                    ));
                } else {
                    self.queue.push_back(InternalOutput::PeerEvent(
                        now,
                        peer_event::Event::Connected(peer_event::Connected {
                            after_ms: (pre_ts - now).as_millis() as u32,
                            remote_ip: ip.to_string(),
                        }),
                    ));
                }
                let (req_id, room, peer, meta, publish, subscribe, mixer) = return_if_none!(self.wait_join.take());
                log::info!("[EndpointInternal] join_room({room}, {peer}) after connected");
                self.join_room(now, req_id, room, peer, meta, publish, subscribe, mixer);
            }
            TransportState::Reconnecting(ip) => {
                log::info!("[EndpointInternal] reconnecting");
                self.queue
                    .push_back(InternalOutput::PeerEvent(now, peer_event::Event::Reconnect(peer_event::Reconnecting { remote_ip: ip.to_string() })));
            }
            TransportState::Disconnected(err) => {
                log::info!("[EndpointInternal] disconnected {:?}", err);
                self.queue.push_back(InternalOutput::PeerEvent(
                    now,
                    peer_event::Event::Disconnected(peer_event::Disconnected { duration_ms: 0, reason: 0 }), //TODO provide correct reason
                ));
                if self.cfg.record {
                    self.queue.push_back(InternalOutput::RecordEvent(now, SessionRecordEvent::Disconnected));
                }
                self.on_shutdown(now);
            }
        }
    }

    fn on_transport_remote_track(&mut self, now: Instant, track: RemoteTrackId, event: RemoteTrackEvent) {
        if let Some((name, _priority, meta)) = event.need_create() {
            log::info!("[EndpointInternal] create remote track {:?}", track);
            let room = self.joined.as_ref().map(|j| j.0);
            let index = self
                .remote_tracks
                .input(&mut self.switcher)
                .add_task(EndpointRemoteTrack::new(room, track, name, meta, self.cfg.record));
            self.remote_tracks_id.insert(track, index);
        }
        let index = return_if_none!(self.remote_tracks_id.get1(&track));
        self.remote_tracks.input(&mut self.switcher).on_event(now, *index, remote_track::Input::Event(event));
    }

    fn on_transport_local_track(&mut self, now: Instant, track: LocalTrackId, event: LocalTrackEvent) {
        if let Some(kind) = event.need_create() {
            log::info!("[EndpointInternal] create local track {:?}", track);
            let room = self.joined.as_ref().map(|j| j.0);
            let index = self.local_tracks.input(&mut self.switcher).add_task(EndpointLocalTrack::new(track, kind, room));
            self.local_tracks_id.insert(track, index);

            // We need to fire event here because local track never removed.
            // Inside local track we only fire attach or detach event
            self.queue.push_back(InternalOutput::PeerEvent(
                now,
                peer_event::Event::LocalTrack(peer_event::LocalTrack {
                    track: *track as i32,
                    kind: Kind::from(kind) as i32,
                }),
            ));
        }
        let index = return_if_none!(self.local_tracks_id.get1(&track));
        self.local_tracks.input(&mut self.switcher).on_event(now, *index, local_track::Input::Event(event));
    }

    fn on_transport_stats(&mut self, _now: Instant, _stats: TransportStats) {}

    #[allow(clippy::too_many_arguments)]
    fn join_room(&mut self, now: Instant, req_id: EndpointReqId, room: RoomId, peer: PeerId, meta: PeerMeta, publish: RoomInfoPublish, subscribe: RoomInfoSubscribe, mixer: Option<AudioMixerConfig>) {
        let room_hash = ClusterRoomHash::generate(&self.cfg.app, &room);
        log::info!("[EndpointInternal] join_room({room}, {peer}), room_hash {room_hash}");
        self.queue.push_back(InternalOutput::RpcRes(req_id, EndpointRes::JoinRoom(Ok(()))));

        self.leave_room(now);

        self.joined = Some((room_hash, room.clone(), peer.clone(), mixer.as_ref().map(|m| m.mode)));
        self.queue
            .push_back(InternalOutput::Cluster(room_hash, ClusterEndpointControl::Join(peer.clone(), meta, publish, subscribe, mixer)));
        if self.cfg.record {
            self.queue
                .push_back(InternalOutput::RecordEvent(now, SessionRecordEvent::JoinRoom(self.cfg.app.app.clone(), room.clone(), peer.clone())));
        }
        self.queue
            .push_back(InternalOutput::PeerEvent(now, peer_event::Event::Join(peer_event::Join { room: room.into(), peer: peer.into() })));

        for (_track_id, index) in self.local_tracks_id.pairs() {
            self.local_tracks.input(&mut self.switcher).on_event(now, index, local_track::Input::JoinRoom(room_hash));
        }

        for (_track_id, index) in self.remote_tracks_id.pairs() {
            self.remote_tracks.input(&mut self.switcher).on_event(now, index, remote_track::Input::JoinRoom(room_hash));
        }
    }

    fn leave_room(&mut self, now: Instant) {
        let (hash, room, peer, _) = return_if_none!(self.joined.take());
        log::info!("[EndpointInternal] leave_room({room}, {peer})");

        for (_track_id, index) in self.local_tracks_id.pairs() {
            self.local_tracks.input(&mut self.switcher).on_event(now, index, local_track::Input::LeaveRoom);
        }

        for (_track_id, index) in self.remote_tracks_id.pairs() {
            self.remote_tracks.input(&mut self.switcher).on_event(now, index, remote_track::Input::LeaveRoom);
        }

        while let Some(task) = self.switcher.current() {
            match task.try_into().expect("Should valid task type") {
                TaskType::BitrateAllocator => self.pop_bitrate_allocator(now),
                TaskType::LocalTracks => self.pop_local_tracks(now),
                TaskType::RemoteTracks => self.pop_remote_tracks(now),
            }
        }

        self.queue.push_back(InternalOutput::Cluster(hash, ClusterEndpointControl::Leave));
        if self.cfg.record {
            self.queue.push_back(InternalOutput::RecordEvent(now, SessionRecordEvent::LeaveRoom));
        }
        self.queue
            .push_back(InternalOutput::PeerEvent(now, peer_event::Event::Leave(peer_event::Leave { room: room.into(), peer: peer.into() })));
    }
}

/// This block is for cluster related events
impl EndpointInternal {
    pub fn on_cluster_event(&mut self, now: Instant, event: ClusterEndpointEvent) {
        match event {
            ClusterEndpointEvent::PeerJoined(peer, meta) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerJoined(peer, meta))),
            ClusterEndpointEvent::PeerLeaved(peer, meta) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerLeaved(peer, meta))),
            ClusterEndpointEvent::TrackStarted(peer, track, meta) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerTrackStarted(peer, track, meta))),
            ClusterEndpointEvent::TrackStopped(peer, track, meta) => self.queue.push_back(InternalOutput::Event(EndpointEvent::PeerTrackStopped(peer, track, meta))),
            ClusterEndpointEvent::AudioMixer(event) => match event {
                ClusterAudioMixerEvent::SlotSet(slot, peer, track) => self
                    .queue
                    .push_back(InternalOutput::Event(EndpointEvent::AudioMixer(EndpointAudioMixerEvent::SlotSet(slot, peer, track)))),
                ClusterAudioMixerEvent::SlotUnset(slot) => self.queue.push_back(InternalOutput::Event(EndpointEvent::AudioMixer(EndpointAudioMixerEvent::SlotUnset(slot)))),
            },
            ClusterEndpointEvent::RemoteTrack(track, event) => self.on_cluster_remote_track(now, track, event),
            ClusterEndpointEvent::LocalTrack(track, event) => self.on_cluster_local_track(now, track, event),
            ClusterEndpointEvent::MessageChannelData(key, from, message) => self.queue.push_back(InternalOutput::Event(EndpointEvent::ChannelMessage(key, from, message))),
        }
    }

    fn on_cluster_remote_track(&mut self, now: Instant, id: RemoteTrackId, event: ClusterRemoteTrackEvent) {
        let index = return_if_none!(self.remote_tracks_id.get1(&id));
        self.remote_tracks.input(&mut self.switcher).on_event(now, *index, remote_track::Input::Cluster(event));
    }

    fn on_cluster_local_track(&mut self, now: Instant, id: LocalTrackId, event: ClusterLocalTrackEvent) {
        let index = return_if_none!(self.local_tracks_id.get1(&id));
        self.local_tracks.input(&mut self.switcher).on_event(now, *index, local_track::Input::Cluster(event));
    }
}

/// This block for internal local and remote track
impl EndpointInternal {
    fn pop_remote_tracks(&mut self, now: Instant) {
        let (index, out) = match return_if_none!(self.remote_tracks.pop_output(now, &mut self.switcher)) {
            TaskGroupOutput::TaskOutput(index, out) => (index, out),
            TaskGroupOutput::OnResourceEmpty => return,
        };
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
            remote_track::Output::Update(kind, priority) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).set_ingress_video_track(id, priority);
                }
            }
            remote_track::Output::Stopped(kind) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).del_ingress_video_track(id);
                }
                self.remote_tracks.input(&mut self.switcher).remove_task(index);
            }
            remote_track::Output::PeerEvent(ts, event) => {
                self.queue.push_back(InternalOutput::PeerEvent(ts, event));
            }
            remote_track::Output::RecordEvent(ts, event) => {
                self.queue.push_back(InternalOutput::RecordEvent(ts, event));
            }
        }
    }

    fn pop_local_tracks(&mut self, now: Instant) {
        let (index, out) = match return_if_none!(self.local_tracks.pop_output(now, &mut self.switcher)) {
            TaskGroupOutput::TaskOutput(index, out) => (index, out),
            TaskGroupOutput::OnResourceEmpty => return,
        };
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
            local_track::Output::Bind(kind, priority) => {
                log::info!("[EndpointInternal] local track bind {kind} priority {priority}");
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).set_egress_video_track(id, priority);
                }
            }
            local_track::Output::Updated(kind, priority) => {
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).set_egress_video_track(id, priority);
                }
            }
            local_track::Output::Unbind(kind) => {
                log::info!("[EndpointInternal] local track unbind {kind}");
                if kind.is_video() {
                    self.bitrate_allocator.input(&mut self.switcher).del_egress_video_track(id);
                }
            }
            local_track::Output::PeerEvent(ts, event) => {
                self.queue.push_back(InternalOutput::PeerEvent(ts, event));
            }
            local_track::Output::OnResourceEmpty => {
                self.local_tracks.input(&mut self.switcher).remove_task(index);
                self.local_tracks_id.remove1(&id);
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

impl Drop for EndpointInternal {
    fn drop(&mut self) {
        assert_eq!(self.queue.len(), 0, "endpoint internal queue should empty on drop");
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        time::Instant,
    };

    use media_server_protocol::{
        endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta},
        protobuf::shared::Kind,
    };
    use media_server_protocol::{multi_tenancy::AppContext, protobuf::cluster_connector::peer_event};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::{
        cluster::{ClusterEndpointControl, ClusterRemoteTrackControl, ClusterRoomHash},
        endpoint::{internal::InternalOutput, EndpointCfg, EndpointReq, EndpointRes},
        transport::{RemoteTrackEvent, TransportEvent, TransportState},
    };

    use super::EndpointInternal;

    #[test_log::test]
    fn test_join_leave_room_success() {
        let app = AppContext::root_app();
        let mut internal = EndpointInternal::new(EndpointCfg {
            app: app.clone(),
            max_egress_bitrate: 2_000_000,
            max_ingress_bitrate: 2_000_000,
            record: false,
        });

        let remote = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let now = Instant::now();
        internal.on_transport_event(now, TransportEvent::State(TransportState::Connecting(remote)));
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(now, peer_event::Event::Connecting(peer_event::Connecting { remote_ip: remote.to_string() })))
        );
        assert_eq!(internal.pop_output(now), None);
        internal.on_transport_event(now, TransportEvent::State(TransportState::Connected(remote)));
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::Connected(peer_event::Connected {
                    remote_ip: remote.to_string(),
                    after_ms: 0
                })
            ))
        );
        assert_eq!(internal.pop_output(now), None);

        let room: RoomId = "room".into();
        let peer: PeerId = "peer".into();
        let meta = PeerMeta { metadata: None, extra_data: None };
        let publish = RoomInfoPublish { peer: true, tracks: true };
        let subscribe = RoomInfoSubscribe { peers: true, tracks: true };
        internal.on_transport_rpc(now, 0.into(), EndpointReq::JoinRoom(room.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone(), None));
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(0.into(), EndpointRes::JoinRoom(Ok(())))));
        let room_hash = ClusterRoomHash::generate(&app, &room);
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(room_hash, ClusterEndpointControl::Join(peer.clone(), meta, publish, subscribe, None)))
        );
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::Join(peer_event::Join {
                    room: room.clone().into(),
                    peer: peer.clone().into()
                })
            ))
        );
        assert_eq!(internal.pop_output(now), None);

        //now start a remote track
        let remote_track_id = 0.into();
        let remote_track_meta = TrackMeta::default_audio();
        internal.on_transport_event(
            now,
            TransportEvent::RemoteTrack(
                remote_track_id,
                RemoteTrackEvent::Started {
                    name: "audio_main".into(),
                    priority: 100.into(),
                    meta: remote_track_meta.clone(),
                },
            ),
        );
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(
                room_hash,
                ClusterEndpointControl::RemoteTrack(remote_track_id, ClusterRemoteTrackControl::Started("audio_main".into(), remote_track_meta.clone()))
            ))
        );
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::RemoteTrackStarted(peer_event::RemoteTrackStarted {
                    track: "audio_main".to_string(),
                    kind: Kind::from(remote_track_meta.kind) as i32,
                }),
            ))
        );
        assert_eq!(internal.pop_output(now), None);

        //now stop remote track
        internal.on_transport_event(now, TransportEvent::RemoteTrack(remote_track_id, RemoteTrackEvent::Ended));
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(
                room_hash,
                ClusterEndpointControl::RemoteTrack(remote_track_id, ClusterRemoteTrackControl::Ended("audio_main".into(), remote_track_meta.clone()))
            ))
        );
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::RemoteTrackEnded(peer_event::RemoteTrackEnded {
                    track: "audio_main".to_string(),
                    kind: Kind::from(remote_track_meta.kind) as i32,
                }),
            ))
        );
        assert_eq!(internal.pop_output(now), None);

        //now leave room should success
        internal.on_transport_rpc(now, 1.into(), EndpointReq::LeaveRoom);
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(1.into(), EndpointRes::LeaveRoom(Ok(())))));
        assert_eq!(internal.pop_output(now), Some(InternalOutput::Cluster(room_hash, ClusterEndpointControl::Leave)));
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(now, peer_event::Event::Leave(peer_event::Leave { room: room.into(), peer: peer.into() })))
        );
        assert_eq!(internal.pop_output(now), None);
    }

    #[test_log::test]
    fn test_join_overwrite_auto_leave() {
        let app = AppContext::root_app();
        let mut internal = EndpointInternal::new(EndpointCfg {
            app: app.clone(),
            max_egress_bitrate: 2_000_000,
            max_ingress_bitrate: 2_000_000,
            record: false,
        });

        let remote = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let now = Instant::now();
        internal.on_transport_event(now, TransportEvent::State(TransportState::Connecting(remote)));
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(now, peer_event::Event::Connecting(peer_event::Connecting { remote_ip: remote.to_string() })))
        );
        assert_eq!(internal.pop_output(now), None);
        internal.on_transport_event(now, TransportEvent::State(TransportState::Connected(remote)));
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::Connected(peer_event::Connected {
                    remote_ip: remote.to_string(),
                    after_ms: 0
                })
            ))
        );
        assert_eq!(internal.pop_output(now), None);

        let room1: RoomId = "room1".into();
        let room1_hash = ClusterRoomHash::generate(&app, &room1);
        let peer: PeerId = "peer".into();
        let meta = PeerMeta { metadata: None, extra_data: None };
        let publish = RoomInfoPublish { peer: true, tracks: true };
        let subscribe = RoomInfoSubscribe { peers: true, tracks: true };
        internal.on_transport_rpc(
            now,
            0.into(),
            EndpointReq::JoinRoom(room1.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone(), None),
        );
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(0.into(), EndpointRes::JoinRoom(Ok(())))));

        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(
                room1_hash,
                ClusterEndpointControl::Join(peer.clone(), meta.clone(), publish.clone(), subscribe.clone(), None),
            ))
        );
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::Join(peer_event::Join {
                    room: room1.clone().into(),
                    peer: peer.clone().into(),
                })
            ))
        );
        assert_eq!(internal.pop_output(now), None);

        //now join other room should success
        let room2: RoomId = "room2".into();
        let room2_hash = ClusterRoomHash::generate(&app, &room2);

        internal.on_transport_rpc(
            now,
            1.into(),
            EndpointReq::JoinRoom(room2.clone(), peer.clone(), meta.clone(), publish.clone(), subscribe.clone(), None),
        );
        assert_eq!(internal.pop_output(now), Some(InternalOutput::RpcRes(1.into(), EndpointRes::JoinRoom(Ok(())))));
        //it will auto leave room1
        assert_eq!(internal.pop_output(now), Some(InternalOutput::Cluster(room1_hash, ClusterEndpointControl::Leave)));
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::Leave(peer_event::Leave {
                    room: room1.clone().into(),
                    peer: peer.clone().into(),
                })
            ))
        );

        //and after that join room2
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::Cluster(
                room2_hash,
                ClusterEndpointControl::Join(peer.clone(), meta.clone(), publish.clone(), subscribe.clone(), None),
            ))
        );
        assert_eq!(
            internal.pop_output(now),
            Some(InternalOutput::PeerEvent(
                now,
                peer_event::Event::Join(peer_event::Join {
                    room: room2.into(),
                    peer: peer.into(),
                })
            ))
        );
        assert_eq!(internal.pop_output(now), None);
    }

    //TODO single local track, join leave room
    //TODO multi local tracks, join leave room
    //TODO single remote track, join leave room

    //TODO multi remote tracks, join leave room
    //TODO both local and remote tracks, join leave room
    //TODO test local and remote stopped must clear resource
    //TODO handle close request
    //TODO handle transport connected
    //TODO handle transport disconnected
}
