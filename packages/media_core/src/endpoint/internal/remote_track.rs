//! RemoteTrack take care about publish local media to sdn, and react with feedback from consumers

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{BitrateControlMode, TrackMeta, TrackName, TrackPriority},
    media::{MediaKind, MediaLayersBitrate},
    protobuf::{cluster_connector::peer_event, shared::Kind},
    record::SessionRecordEvent,
    transport::{RemoteTrackId, RpcError},
};
use sans_io_runtime::{return_if_none, Task, TaskSwitcherChild};

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

pub struct EndpointRemoteTrack {
    id: RemoteTrackId,
    meta: TrackMeta,
    room: Option<ClusterRoomHash>,
    name: TrackName,
    queue: VecDeque<Output>,
    allocate_bitrate: Option<u64>,
    /// This is for storing current stream layers, everytime key-frame arrived we will set this if it not set
    last_layers: Option<MediaLayersBitrate>,
    cluster_bitrate_limit: Option<(u64, u64)>,
    record: bool,
    shutdown: bool,
}

impl EndpointRemoteTrack {
    pub fn new(room: Option<ClusterRoomHash>, id: RemoteTrackId, name: TrackName, meta: TrackMeta, record: bool) -> Self {
        log::info!("[EndpointRemoteTrack] created with room {:?} meta {:?}", room, meta);
        Self {
            id,
            meta,
            room,
            name,
            queue: VecDeque::new(),
            allocate_bitrate: None,
            last_layers: None,
            cluster_bitrate_limit: None,
            record,
            shutdown: false,
        }
    }

    fn on_join_room(&mut self, now: Instant, room: ClusterRoomHash) {
        assert_eq!(self.room, None);
        let name = self.name.clone();
        self.room = Some(room);
        log::info!("[EndpointRemoteTrack] join room {room} as name {name}");
        log::info!("[EndpointRemoteTrack] started as name {name} after join room");
        self.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Started(name.clone(), self.meta.clone())));
        if self.record {
            self.queue
                .push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStarted(self.id, name.clone(), self.meta.clone())));
        }
        self.queue.push_back(Output::PeerEvent(
            now,
            peer_event::Event::RemoteTrackStarted(peer_event::RemoteTrackStarted {
                track: name.into(),
                kind: Kind::from(self.meta.kind) as i32,
            }),
        ));
    }

    fn on_leave_room(&mut self, now: Instant) {
        let room = self.room.take().expect("Must have room here");
        let name = self.name.clone();
        log::info!("[EndpointRemoteTrack] leave room {room} as name {name}");
        log::info!("[EndpointRemoteTrack] stopped as name {name} after leave room");
        self.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Ended(name.clone(), self.meta.clone())));
        if self.record {
            self.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStopped(self.id)));
        }
        self.queue.push_back(Output::PeerEvent(
            now,
            peer_event::Event::RemoteTrackEnded(peer_event::RemoteTrackEnded {
                track: name.into(),
                kind: Kind::from(self.meta.kind) as i32,
            }),
        ));
    }

    fn on_cluster_event(&mut self, _now: Instant, event: ClusterRemoteTrackEvent) {
        match event {
            ClusterRemoteTrackEvent::RequestKeyFrame => self.queue.push_back(Output::Event(EndpointRemoteTrackEvent::RequestKeyFrame)),
            ClusterRemoteTrackEvent::LimitBitrate { min, max } => {
                self.cluster_bitrate_limit = Some((min, max));
                if self.meta.control.eq(&BitrateControlMode::DynamicConsumers) {
                    if let Some((min, max)) = self.calc_limit_bitrate() {
                        self.queue.push_back(Output::Event(EndpointRemoteTrackEvent::LimitBitrateBps { min, max }));
                    }
                }
            }
        }
    }

    fn on_transport_event(&mut self, now: Instant, event: RemoteTrackEvent) {
        match event {
            RemoteTrackEvent::Started { name, priority, meta } => {
                let room = return_if_none!(self.room.as_ref());
                log::info!("[EndpointRemoteTrack] started as name {name} in room {room}");
                self.queue.push_back(Output::Cluster(*room, ClusterRemoteTrackControl::Started(name.clone().into(), self.meta.clone())));
                self.queue.push_back(Output::Started(self.meta.kind, priority));
                if self.record {
                    self.queue
                        .push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStarted(self.id, name.clone().into(), self.meta.clone())));
                }
                self.queue.push_back(Output::PeerEvent(
                    now,
                    peer_event::Event::RemoteTrackStarted(peer_event::RemoteTrackStarted {
                        track: name,
                        kind: Kind::from(meta.kind) as i32,
                    }),
                ));
            }
            RemoteTrackEvent::Paused => {}
            RemoteTrackEvent::Resumed => {}
            RemoteTrackEvent::Media(mut media) => {
                //TODO clear self.last_layer if switched to new track
                if media.layers.is_some() {
                    log::debug!("[EndpointRemoteTrack] on layers info {:?}", media.layers);
                    self.last_layers.clone_from(&media.layers);
                }

                // We restore last_layer if key frame not contain for allow consumers fast switching
                if media.meta.is_video_key() && media.layers.is_none() && self.last_layers.is_some() {
                    log::debug!("[EndpointRemoteTrack] set layers info to key-frame {:?}", media.layers);
                    media.layers.clone_from(&self.last_layers);
                }

                if self.record {
                    self.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackMedia(self.id, media.clone())));
                }

                let room = return_if_none!(self.room.as_ref());
                self.queue.push_back(Output::Cluster(*room, ClusterRemoteTrackControl::Media(media)));
            }
            RemoteTrackEvent::Ended => {
                let name = self.name.clone();
                let room = return_if_none!(self.room.as_ref());
                log::info!("[EndpointRemoteTrack] stopped with name {name} in room {room}");
                self.queue.push_back(Output::Cluster(*room, ClusterRemoteTrackControl::Ended(name.clone(), self.meta.clone())));
                if self.record {
                    self.queue.push_back(Output::RecordEvent(now, SessionRecordEvent::TrackStopped(self.id)));
                }
                self.queue.push_back(Output::PeerEvent(
                    now,
                    peer_event::Event::RemoteTrackEnded(peer_event::RemoteTrackEnded {
                        track: name.into(),
                        kind: Kind::from(self.meta.kind) as i32,
                    }),
                ));
                self.shutdown = true;
            }
        }
    }

    fn on_rpc_req(&mut self, _now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq) {
        match req {
            EndpointRemoteTrackReq::Config(config) => {
                if *config.priority == 0 {
                    log::warn!("[EndpointRemoteTrack] view with invalid priority");
                    self.queue
                        .push_back(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Err(RpcError::new2(EndpointErrors::RemoteTrackInvalidPriority)))));
                } else {
                    self.meta.control = config.control;
                    self.queue.push_back(Output::RpcRes(req_id, EndpointRemoteTrackRes::Config(Ok(()))));
                    self.queue.push_back(Output::Update(self.meta.kind, config.priority));
                }
            }
        }
    }

    fn on_bitrate_allocation_action(&mut self, _now: Instant, action: IngressAction) {
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

    fn calc_limit_bitrate(&self) -> Option<(u64, u64)> {
        let cluster_limit = self.meta.control.eq(&BitrateControlMode::DynamicConsumers).then_some(self.cluster_bitrate_limit).flatten();
        match (self.allocate_bitrate, cluster_limit) {
            (Some(b1), Some((min, max))) => Some((min.min(b1), max.min(b1))),
            (Some(b1), None) => Some((b1, b1)),
            (None, Some((min, max))) => Some((min, max)),
            (None, None) => None,
        }
    }
}

impl Task<Input, Output> for EndpointRemoteTrack {
    fn on_tick(&mut self, _now: Instant) {}

    fn on_event(&mut self, now: Instant, input: Input) {
        match input {
            Input::JoinRoom(room) => self.on_join_room(now, room),
            Input::LeaveRoom => self.on_leave_room(now),
            Input::Cluster(event) => self.on_cluster_event(now, event),
            Input::Event(event) => self.on_transport_event(now, event),
            Input::RpcReq(req_id, req) => self.on_rpc_req(now, req_id, req),
            Input::BitrateAllocation(action) => self.on_bitrate_allocation_action(now, action),
        }
    }

    fn on_shutdown(&mut self, now: Instant) {
        if self.shutdown {
            return;
        }
        self.shutdown = true;
        if self.room.is_some() {
            self.on_leave_room(now);
        }
    }
}

impl TaskSwitcherChild<Output> for EndpointRemoteTrack {
    type Time = Instant;

    fn is_empty(&self) -> bool {
        self.shutdown && self.queue.is_empty()
    }

    fn empty_event(&self) -> Output {
        Output::Stopped(self.meta.kind)
    }

    fn pop_output(&mut self, _now: Instant) -> Option<Output> {
        self.queue.pop_front()
    }
}

impl Drop for EndpointRemoteTrack {
    fn drop(&mut self) {
        assert_eq!(self.queue.len(), 0, "remote track queue should empty on drop");
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use media_server_protocol::{
        endpoint::{TrackMeta, TrackName},
        protobuf::{cluster_connector::peer_event, shared::Kind},
    };
    use sans_io_runtime::{Task, TaskSwitcherChild};

    use crate::{cluster::ClusterRemoteTrackControl, transport::RemoteTrackEvent};

    use super::{EndpointRemoteTrack, Input, Output};

    #[test_log::test]
    fn start_in_room() {
        let room = 0.into();
        let track_name = TrackName::from("audio_main");
        let track_id = 1.into();
        let track_priority = 2.into();
        let meta = TrackMeta::default_audio();
        let now = Instant::now();
        let mut track = EndpointRemoteTrack::new(Some(room), track_id, track_name.clone(), meta.clone(), false);
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
        assert_eq!(track.pop_output(now), None);
        //we dont need Output::Stopped here, it will be fired with TaskSwitcherChild::pop_output with is_empty true
        assert_eq!(track.is_empty(), true);
    }

    //TODO start not in room
    //TODO stop in room
    //TODO stop not in room
    //TODO switched room need fire events
    //TODO send media to cluster
    //TODO handle key-frame-request feedback
    //TODO handle bitrate limit feedback
}
