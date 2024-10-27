//! RemoteTrack take care about publish local media to sdn, and react with feedback from consumers
//!
//! State Machine Diagram:
//! ```ascii
//!                        JoinRoom
//!     ┌─────────────────────────────────┐
//!     │                                 ▼
//! ┌───────────┐                    ┌─────────┐
//! │  Waiting  │                    │         │
//! │  JoinRoom │◄───────────────────│ InRoom  │
//! └───────────┘     LeaveRoom      │         │
//!       │                          └─────────┘
//!       │                               │
//!       │           TrackEnded         │
//!       │                              │
//!       │           ┌─────────┐        │
//!       └──────────►│         │◄───────┘
//!                   │ Stopped │
//!                   │         │
//!                   └─────────┘
//! ```
//!
//! State Transitions:
//! - WaitingJoinRoom -> InRoom: via JoinRoom event
//! - InRoom -> WaitingJoinRoom: via LeaveRoom event
//! - WaitingJoinRoom/InRoom -> Stopped: via TrackEnded event
//! - Stopped: Terminal state, no transitions out
//!

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{BitrateControlMode, TrackMeta, TrackName, TrackPriority},
    media::{MediaKind, MediaLayersBitrate, MediaPacket},
    protobuf::{cluster_connector::peer_event, shared::Kind},
    record::SessionRecordEvent,
    transport::{RemoteTrackId, RpcError},
};
use sans_io_runtime::{Task, TaskSwitcherChild};

use crate::{
    cluster::{ClusterRemoteTrackControl, ClusterRemoteTrackEvent, ClusterRoomHash},
    endpoint::{EndpointRemoteTrackEvent, EndpointRemoteTrackReq, EndpointRemoteTrackRes, EndpointReqId},
    errors::EndpointErrors,
    transport::RemoteTrackEvent,
};

use super::bitrate_allocator::IngressAction;

pub enum Input {
    JoinRoom(ClusterRoomHash),
    LeaveRoom,
    Cluster(ClusterRemoteTrackEvent),
    Event(RemoteTrackEvent),
    RpcReq(EndpointReqId, EndpointRemoteTrackReq),
    BitrateAllocation(IngressAction),
}

#[derive(Debug, PartialEq)]
pub enum Output {
    Event(EndpointRemoteTrackEvent),
    Cluster(ClusterRoomHash, ClusterRemoteTrackControl),
    PeerEvent(Instant, peer_event::Event),
    RecordEvent(Instant, SessionRecordEvent),
    RpcRes(EndpointReqId, EndpointRemoteTrackRes),
    Started(MediaKind, TrackPriority),
    Update(MediaKind, TrackPriority),
    Stopped(MediaKind),
}

struct StateContext {
    id: RemoteTrackId,
    name: TrackName,
    meta: TrackMeta,
    priority: TrackPriority,
    queue: VecDeque<Output>,
    /// This is for storing current stream layers, everytime key-frame arrived we will set this if it not set
    last_layers: Option<MediaLayersBitrate>,
    cluster_bitrate_limit: Option<(u64, u64)>,
    record: bool,
    allocate_bitrate: Option<u64>,
    next_state: Option<State>,
}

impl StateContext {
    fn calc_limit_bitrate(&self) -> Option<(u64, u64)> {
        let cluster_limit = self.meta.control.eq(&BitrateControlMode::DynamicConsumers).then_some(self.cluster_bitrate_limit).flatten();
        match (self.allocate_bitrate, cluster_limit) {
            (Some(b1), Some((min, max))) => Some((min.min(b1), max.min(b1))),
            (Some(b1), None) => Some((b1, b1)),
            (None, Some((min, max))) => Some((min, max)),
            (None, None) => None,
        }
    }

    fn limit_bitrate(&mut self, action: IngressAction) {
        match action {
            IngressAction::SetBitrate(bitrate) => {
                log::info!("[EndpointRemoteTrack] on allocation bitrate {bitrate}");
                self.allocate_bitrate = Some(bitrate);
                if let Some((min, max)) = self.calc_limit_bitrate() {
                    self.queue.push_back(Output::Event(EndpointRemoteTrackEvent::LimitBitrateBps { min, max }))
                }
            }
        }
    }
}

trait StateLogic {
    fn on_join_room(&mut self, ctx: &mut StateContext, now: Instant, room: ClusterRoomHash);
    fn on_leave_room(&mut self, ctx: &mut StateContext, now: Instant);
    fn on_track_started(&mut self, ctx: &mut StateContext, now: Instant);
    fn on_track_media(&mut self, ctx: &mut StateContext, now: Instant, media: MediaPacket);
    fn on_track_ended(&mut self, ctx: &mut StateContext, now: Instant);
    fn on_rpc_req(&mut self, ctx: &mut StateContext, now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq);
    fn on_cluster_event(&mut self, ctx: &mut StateContext, now: Instant, event: ClusterRemoteTrackEvent);
    fn on_bitrate_allocation(&mut self, ctx: &mut StateContext, now: Instant, action: IngressAction);
    fn pop_output(&mut self, ctx: &mut StateContext, now: Instant) -> Option<Output>;
}

struct WaitingJoinRoom;
struct InRoom {
    room: ClusterRoomHash,
}
struct Stopped {
    waiting: bool,
}

enum State {
    WaitingJoinRoom(WaitingJoinRoom),
    InRoom(InRoom),
    Stopped(Stopped),
}
impl StateLogic for State {
    fn on_join_room(&mut self, ctx: &mut StateContext, now: Instant, room: ClusterRoomHash) {
        match self {
            State::WaitingJoinRoom(state) => state.on_join_room(ctx, now, room),
            State::InRoom(state) => state.on_join_room(ctx, now, room),
            State::Stopped(state) => state.on_join_room(ctx, now, room),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn on_leave_room(&mut self, ctx: &mut StateContext, now: Instant) {
        match self {
            State::WaitingJoinRoom(state) => state.on_leave_room(ctx, now),
            State::InRoom(state) => state.on_leave_room(ctx, now),
            State::Stopped(state) => state.on_leave_room(ctx, now),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn on_track_started(&mut self, ctx: &mut StateContext, now: Instant) {
        match self {
            State::WaitingJoinRoom(state) => state.on_track_started(ctx, now),
            State::InRoom(state) => state.on_track_started(ctx, now),
            State::Stopped(state) => state.on_track_started(ctx, now),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn on_track_media(&mut self, ctx: &mut StateContext, now: Instant, media: MediaPacket) {
        match self {
            State::WaitingJoinRoom(state) => state.on_track_media(ctx, now, media),
            State::InRoom(state) => state.on_track_media(ctx, now, media),
            State::Stopped(state) => state.on_track_media(ctx, now, media),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn on_track_ended(&mut self, ctx: &mut StateContext, now: Instant) {
        match self {
            State::WaitingJoinRoom(state) => state.on_track_ended(ctx, now),
            State::InRoom(state) => state.on_track_ended(ctx, now),
            State::Stopped(state) => state.on_track_ended(ctx, now),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn on_rpc_req(&mut self, ctx: &mut StateContext, now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq) {
        match self {
            State::WaitingJoinRoom(state) => state.on_rpc_req(ctx, now, req_id, req),
            State::InRoom(state) => state.on_rpc_req(ctx, now, req_id, req),
            State::Stopped(state) => state.on_rpc_req(ctx, now, req_id, req),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn on_cluster_event(&mut self, ctx: &mut StateContext, now: Instant, event: ClusterRemoteTrackEvent) {
        match self {
            State::WaitingJoinRoom(state) => state.on_cluster_event(ctx, now, event),
            State::InRoom(state) => state.on_cluster_event(ctx, now, event),
            State::Stopped(state) => state.on_cluster_event(ctx, now, event),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn on_bitrate_allocation(&mut self, ctx: &mut StateContext, now: Instant, action: IngressAction) {
        match self {
            State::WaitingJoinRoom(state) => state.on_bitrate_allocation(ctx, now, action),
            State::InRoom(state) => state.on_bitrate_allocation(ctx, now, action),
            State::Stopped(state) => state.on_bitrate_allocation(ctx, now, action),
        }

        if let Some(next_state) = ctx.next_state.take() {
            *self = next_state;
        }
    }

    fn pop_output(&mut self, ctx: &mut StateContext, now: Instant) -> Option<Output> {
        match self {
            State::WaitingJoinRoom(state) => state.pop_output(ctx, now),
            State::InRoom(state) => state.pop_output(ctx, now),
            State::Stopped(state) => state.pop_output(ctx, now),
        }
    }
}

impl StateLogic for WaitingJoinRoom {
    fn on_join_room(&mut self, ctx: &mut StateContext, now: Instant, room: ClusterRoomHash) {
        log::info!("[EndpointRemoteTrack] join room {room}");
        let name = ctx.name.clone();
        log::info!("[EndpointRemoteTrack] started as name {name} after join room");
        ctx.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Started(name.clone(), ctx.meta.clone())));
        if ctx.record {
            ctx.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStarted(ctx.id, name.clone(), ctx.meta.clone())));
        }
        ctx.queue.push_back(Output::PeerEvent(
            now,
            peer_event::Event::RemoteTrackStarted(peer_event::RemoteTrackStarted {
                track: name.into(),
                kind: Kind::from(ctx.meta.kind) as i32,
            }),
        ));
        ctx.next_state = Some(State::InRoom(InRoom { room }));
    }

    fn on_leave_room(&mut self, _ctx: &mut StateContext, _now: Instant) {
        log::warn!("[EndpointRemoteTrack] leave room but not in room");
    }

    fn on_track_started(&mut self, ctx: &mut StateContext, _now: Instant) {
        ctx.queue.push_back(Output::Started(ctx.meta.kind, ctx.priority));
    }

    fn on_track_media(&mut self, _ctx: &mut StateContext, _now: Instant, _media: MediaPacket) {}

    fn on_track_ended(&mut self, ctx: &mut StateContext, _now: Instant) {
        ctx.next_state = Some(State::Stopped(Stopped { waiting: true }));
    }

    fn on_rpc_req(&mut self, ctx: &mut StateContext, _now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq) {
        match req {
            EndpointRemoteTrackReq::Config(config) => {
                if *config.priority == 0 {
                    log::warn!("[EndpointRemoteTrack] view with invalid priority");
                    ctx.queue
                        .push_back(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Err(RpcError::new2(EndpointErrors::RemoteTrackInvalidPriority)))));
                } else {
                    ctx.meta.control = config.control;
                    ctx.queue.push_back(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Ok(()))));
                    ctx.queue.push_back(Output::Update(ctx.meta.kind, config.priority));
                }
            }
        }
    }

    fn on_cluster_event(&mut self, _ctx: &mut StateContext, _now: Instant, _event: ClusterRemoteTrackEvent) {
        log::warn!("[EndpointRemoteTrack] on cluster event but not in room");
    }

    fn on_bitrate_allocation(&mut self, ctx: &mut StateContext, _now: Instant, action: IngressAction) {
        ctx.limit_bitrate(action);
    }

    fn pop_output(&mut self, ctx: &mut StateContext, _now: Instant) -> Option<Output> {
        ctx.queue.pop_front()
    }
}

impl StateLogic for InRoom {
    fn on_join_room(&mut self, _ctx: &mut StateContext, _now: Instant, _room: ClusterRoomHash) {
        log::warn!("[EndpointRemoteTrack] join room but already in room");
    }

    fn on_leave_room(&mut self, ctx: &mut StateContext, now: Instant) {
        let room = self.room;
        log::info!("[EndpointRemoteTrack] leave room {room}");
        let name = ctx.name.clone();
        log::info!("[EndpointRemoteTrack] stopped as name {name} after leave room");
        ctx.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Ended(name.clone(), ctx.meta.clone())));
        if ctx.record {
            ctx.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStopped(ctx.id)));
        }
        ctx.queue.push_back(Output::PeerEvent(
            now,
            peer_event::Event::RemoteTrackEnded(peer_event::RemoteTrackEnded {
                track: name.into(),
                kind: Kind::from(ctx.meta.kind) as i32,
            }),
        ));
        ctx.next_state = Some(State::WaitingJoinRoom(WaitingJoinRoom));
    }

    fn on_track_started(&mut self, ctx: &mut StateContext, now: Instant) {
        let room = self.room;
        let name = ctx.name.clone();
        log::info!("[EndpointRemoteTrack] started as name {name} with room {room}");
        ctx.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Started(name.clone(), ctx.meta.clone())));
        ctx.queue.push_back(Output::Started(ctx.meta.kind, ctx.priority));
        if ctx.record {
            ctx.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStarted(ctx.id, name.clone(), ctx.meta.clone())));
        }
        ctx.queue.push_back(Output::PeerEvent(
            now,
            peer_event::Event::RemoteTrackStarted(peer_event::RemoteTrackStarted {
                track: name.into(),
                kind: Kind::from(ctx.meta.kind) as i32,
            }),
        ));
    }

    fn on_track_media(&mut self, ctx: &mut StateContext, now: Instant, mut media: MediaPacket) {
        // We restore last_layer if key frame not contain for allow consumers fast switching
        if media.meta.is_video_key() && media.layers.is_none() && ctx.last_layers.is_some() {
            log::debug!("[EndpointRemoteTrack] set layers info to key-frame {:?}", media.layers);
            media.layers.clone_from(&ctx.last_layers);
        }

        if ctx.record {
            ctx.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackMedia(ctx.id, media.clone())));
        }

        ctx.queue.push_back(Output::Cluster(self.room, ClusterRemoteTrackControl::Media(media)));
    }

    fn on_track_ended(&mut self, ctx: &mut StateContext, now: Instant) {
        let room = self.room;
        let name = ctx.name.clone();
        log::info!("[EndpointRemoteTrack] stopped with name {name} in room {room}");
        ctx.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Ended(name.clone(), ctx.meta.clone())));
        if ctx.record {
            ctx.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStopped(ctx.id)));
        }
        ctx.queue.push_back(Output::PeerEvent(
            now,
            peer_event::Event::RemoteTrackEnded(peer_event::RemoteTrackEnded {
                track: name.into(),
                kind: Kind::from(ctx.meta.kind) as i32,
            }),
        ));
        ctx.next_state = Some(State::Stopped(Stopped { waiting: true }));
    }

    fn on_rpc_req(&mut self, ctx: &mut StateContext, _now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq) {
        match req {
            EndpointRemoteTrackReq::Config(config) => {
                if *config.priority == 0 {
                    log::warn!("[EndpointRemoteTrack] view with invalid priority");
                    ctx.queue
                        .push_back(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Err(RpcError::new2(EndpointErrors::RemoteTrackInvalidPriority)))));
                } else {
                    ctx.meta.control = config.control;
                    ctx.queue.push_back(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Ok(()))));
                    ctx.queue.push_back(Output::Update(ctx.meta.kind, config.priority));
                }
            }
        }
    }

    fn on_cluster_event(&mut self, ctx: &mut StateContext, _now: Instant, event: ClusterRemoteTrackEvent) {
        match event {
            ClusterRemoteTrackEvent::RequestKeyFrame => ctx.queue.push_back(Output::Event(EndpointRemoteTrackEvent::RequestKeyFrame)),
            ClusterRemoteTrackEvent::LimitBitrate { min, max } => {
                ctx.cluster_bitrate_limit = Some((min, max));
                if ctx.meta.control.eq(&BitrateControlMode::DynamicConsumers) {
                    if let Some((min, max)) = ctx.calc_limit_bitrate() {
                        ctx.queue.push_back(Output::Event(EndpointRemoteTrackEvent::LimitBitrateBps { min, max }));
                    }
                }
            }
        }
    }

    fn on_bitrate_allocation(&mut self, ctx: &mut StateContext, _now: Instant, action: IngressAction) {
        ctx.limit_bitrate(action);
    }

    fn pop_output(&mut self, ctx: &mut StateContext, _now: Instant) -> Option<Output> {
        ctx.queue.pop_front()
    }
}

impl StateLogic for Stopped {
    fn on_join_room(&mut self, _ctx: &mut StateContext, _now: Instant, _room: ClusterRoomHash) {
        log::warn!("[EndpointRemoteTrack] join room but stopped");
    }

    fn on_leave_room(&mut self, _ctx: &mut StateContext, _now: Instant) {
        log::warn!("[EndpointRemoteTrack] leave room but stopped");
    }

    fn on_track_started(&mut self, _ctx: &mut StateContext, _now: Instant) {
        log::warn!("[EndpointRemoteTrack] track started but stopped");
    }

    fn on_track_media(&mut self, _ctx: &mut StateContext, _now: Instant, _media: MediaPacket) {
        log::warn!("[EndpointRemoteTrack] track media but stopped");
    }

    fn on_track_ended(&mut self, _ctx: &mut StateContext, _now: Instant) {
        log::warn!("[EndpointRemoteTrack] track ended but stopped");
    }

    fn on_rpc_req(&mut self, ctx: &mut StateContext, _now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq) {
        match req {
            EndpointRemoteTrackReq::Config(_config) => {
                ctx.queue
                    .push_back(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Err(RpcError::new2(EndpointErrors::RemoteTrackStopped)))));
            }
        }
    }

    fn on_cluster_event(&mut self, _ctx: &mut StateContext, _now: Instant, _event: ClusterRemoteTrackEvent) {
        log::warn!("[EndpointRemoteTrack] on cluster event but stopped");
    }

    fn on_bitrate_allocation(&mut self, _ctx: &mut StateContext, _now: Instant, _action: IngressAction) {
        log::warn!("[EndpointRemoteTrack] on bitrate allocation but stopped");
    }

    fn pop_output(&mut self, ctx: &mut StateContext, _now: Instant) -> Option<Output> {
        if ctx.queue.is_empty() && self.waiting {
            self.waiting = false;
            // We must send Stopped at last, if not we missed some event
            Some(Output::Stopped(ctx.meta.kind))
        } else {
            ctx.queue.pop_front()
        }
    }
}

pub struct EndpointRemoteTrack {
    ctx: StateContext,
    state: State,
}

impl EndpointRemoteTrack {
    pub fn new(room: Option<ClusterRoomHash>, id: RemoteTrackId, name: TrackName, priority: TrackPriority, meta: TrackMeta, record: bool) -> Self {
        log::info!("[EndpointRemoteTrack] created with room {:?} meta {:?}", room, meta);
        Self {
            ctx: StateContext {
                id,
                name,
                meta,
                priority,
                queue: Default::default(),
                last_layers: None,
                cluster_bitrate_limit: None,
                record,
                next_state: None,
                allocate_bitrate: None,
            },
            state: if let Some(room) = room {
                State::InRoom(InRoom { room })
            } else {
                State::WaitingJoinRoom(WaitingJoinRoom)
            },
        }
    }
}

impl Task<Input, Output> for EndpointRemoteTrack {
    fn on_tick(&mut self, _now: Instant) {}

    fn on_event(&mut self, now: Instant, input: Input) {
        match input {
            Input::JoinRoom(room) => self.state.on_join_room(&mut self.ctx, now, room),
            Input::LeaveRoom => self.state.on_leave_room(&mut self.ctx, now),
            Input::Cluster(event) => self.state.on_cluster_event(&mut self.ctx, now, event),
            Input::Event(event) => match event {
                RemoteTrackEvent::Started { .. } => {
                    self.state.on_track_started(&mut self.ctx, now);
                }
                RemoteTrackEvent::Paused => {}
                RemoteTrackEvent::Resumed => {}
                RemoteTrackEvent::Media(media) => {
                    //TODO clear self.last_layer if switched to new track
                    if media.layers.is_some() {
                        log::debug!("[EndpointRemoteTrack] on layers info {:?}", media.layers);
                        self.ctx.last_layers.clone_from(&media.layers);
                    }

                    self.state.on_track_media(&mut self.ctx, now, media);
                }
                RemoteTrackEvent::Ended => {
                    self.state.on_track_ended(&mut self.ctx, now);
                }
            },
            Input::RpcReq(req_id, req) => self.state.on_rpc_req(&mut self.ctx, now, req_id, req),
            Input::BitrateAllocation(action) => self.state.on_bitrate_allocation(&mut self.ctx, now, action),
        }
    }

    fn on_shutdown(&mut self, _now: Instant) {}
}

impl TaskSwitcherChild<Output> for EndpointRemoteTrack {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<Output> {
        self.state.pop_output(&mut self.ctx, now)
    }
}

impl Drop for EndpointRemoteTrack {
    fn drop(&mut self) {
        assert_eq!(self.ctx.queue.len(), 0, "remote track queue should empty on drop");
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use media_server_protocol::{
        endpoint::{BitrateControlMode, TrackMeta, TrackName},
        media::MediaKind,
        protobuf::{cluster_connector::peer_event, shared::Kind},
        transport::RpcError,
    };
    use sans_io_runtime::{Task, TaskSwitcherChild};

    use crate::{
        cluster::ClusterRemoteTrackControl,
        endpoint::{EndpointRemoteTrackConfig, EndpointRemoteTrackReq, EndpointRemoteTrackRes, EndpointReqId},
        errors::EndpointErrors,
        transport::RemoteTrackEvent,
    };

    use super::{EndpointRemoteTrack, Input, Output};

    #[test]
    fn start_in_room() {
        let room = 0.into();
        let track_name = TrackName::from("audio_main");
        let track_id = 1.into();
        let track_priority = 2.into();
        let meta = TrackMeta::default_audio();
        let now = Instant::now();
        let mut track = EndpointRemoteTrack::new(Some(room), track_id, track_name.clone(), track_priority, meta.clone(), false);
        assert_eq!(track.pop_output(now), None);

        track.on_event(
            now,
            Input::Event(RemoteTrackEvent::Started {
                name: track_name.clone().into(),
                priority: track_priority,
                meta: meta.clone(),
            }),
        );

        assert_eq!(track.pop_output(now), Some(Output::Cluster(room, ClusterRemoteTrackControl::Started(track_name.clone(), meta.clone()))));
        assert_eq!(track.pop_output(now), Some(Output::Started(meta.kind, track_priority)));
        assert_eq!(
            track.pop_output(now),
            Some(Output::PeerEvent(
                now,
                peer_event::Event::RemoteTrackStarted(peer_event::RemoteTrackStarted {
                    track: track_name.clone().into(),
                    kind: Kind::from(meta.kind) as i32,
                }),
            ))
        );
        assert_eq!(track.pop_output(now), None);

        //now leave room
        let now = now + Duration::from_secs(1);
        track.on_event(now, Input::Event(RemoteTrackEvent::Ended));

        assert_eq!(track.pop_output(now), Some(Output::Cluster(room, ClusterRemoteTrackControl::Ended(track_name.clone(), meta.clone()))));
        assert_eq!(
            track.pop_output(now),
            Some(Output::PeerEvent(
                now,
                peer_event::Event::RemoteTrackEnded(peer_event::RemoteTrackEnded {
                    track: track_name.clone().into(),
                    kind: Kind::from(meta.kind) as i32,
                }),
            ))
        );
        //we need Output::Stopped at last
        assert_eq!(track.pop_output(now), Some(Output::Stopped(meta.kind)));
        assert_eq!(track.pop_output(now), None);
    }

    #[test]
    fn should_wait_for_stopped() {
        let name = TrackName::from("audio_main");
        let id = 1.into();
        let priority = 2.into();
        let meta = TrackMeta::default_audio();
        let now = Instant::now();
        let mut track = EndpointRemoteTrack::new(None, id, name.clone(), priority, meta.clone(), false);

        track.on_event(
            now,
            Input::Event(RemoteTrackEvent::Started {
                name: name.clone().into(),
                priority,
                meta,
            }),
        );
        assert_eq!(track.pop_output(now), Some(Output::Started(MediaKind::Audio, priority)));
        assert_eq!(track.pop_output(now), None);
        track.on_event(now, Input::Event(RemoteTrackEvent::Ended));

        let req_id = EndpointReqId(0);
        track.on_event(
            now,
            Input::RpcReq(
                req_id,
                EndpointRemoteTrackReq::Config(EndpointRemoteTrackConfig {
                    priority,
                    control: BitrateControlMode::DynamicConsumers,
                }),
            ),
        );
        assert_eq!(
            track.pop_output(now),
            Some(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Err(RpcError::new2(EndpointErrors::RemoteTrackStopped)))))
        );
        //we need Output::Stopped at last
        assert_eq!(track.pop_output(now), Some(Output::Stopped(MediaKind::Audio)));
        assert_eq!(track.pop_output(now), None);
    }

    //TODO start not in room
    //TODO stop in room
    //TODO stop not in room
    //TODO switched room need fire events
    //TODO send media to cluster
    //TODO handle key-frame-request feedback
    //TODO handle bitrate limit feedback
}
