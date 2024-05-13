//! RemoteTrack take care about publish local media to sdn, and react with feedback from consumers

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{BitrateControlMode, TrackMeta, TrackName, TrackPriority},
    media::{MediaKind, MediaLayersBitrate},
};
use sans_io_runtime::{return_if_none, Task, TaskSwitcherChild};

use crate::{
    cluster::{ClusterRemoteTrackControl, ClusterRemoteTrackEvent, ClusterRoomHash},
    endpoint::{EndpointRemoteTrackEvent, EndpointRemoteTrackReq, EndpointRemoteTrackRes, EndpointReqId},
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

#[derive(Debug)]
pub enum Output {
    Event(EndpointRemoteTrackEvent),
    Cluster(ClusterRoomHash, ClusterRemoteTrackControl),
    RpcRes(EndpointReqId, EndpointRemoteTrackRes),
    Started(MediaKind, TrackPriority),
    Stopped(MediaKind),
}

pub struct EndpointRemoteTrack {
    meta: TrackMeta,
    room: Option<ClusterRoomHash>,
    name: Option<String>,
    queue: VecDeque<Output>,
    allocate_bitrate: Option<u64>,
    /// This is for storing current stream layers, everytime key-frame arrived we will set this if it not set
    last_layers: Option<MediaLayersBitrate>,
    cluster_bitrate_limit: Option<(u64, u64)>,
}

impl EndpointRemoteTrack {
    pub fn new(room: Option<ClusterRoomHash>, meta: TrackMeta) -> Self {
        log::info!("[EndpointRemoteTrack] created with room {:?} meta {:?}", room, meta);
        Self {
            meta,
            room,
            name: None,
            queue: VecDeque::new(),
            allocate_bitrate: None,
            last_layers: None,
            cluster_bitrate_limit: None,
        }
    }

    fn on_join_room(&mut self, _now: Instant, room: ClusterRoomHash) {
        assert_eq!(self.room, None);
        self.room = Some(room);
        log::info!("[EndpointRemoteTrack] join room {room}");
        let name = return_if_none!(self.name.clone());
        log::info!("[EndpointRemoteTrack] started as name {name} after join room");
        self.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Started(TrackName(name), self.meta.clone())));
    }
    fn on_leave_room(&mut self, _now: Instant) {
        let room = self.room.take().expect("Must have room here");
        log::info!("[EndpointRemoteTrack] leave room {room}");
        let name = return_if_none!(self.name.as_ref());
        log::info!("[EndpointRemoteTrack] stopped as name {name} after leave room");
        self.queue.push_back(Output::Cluster(room, ClusterRemoteTrackControl::Ended));
    }

    fn on_cluster_event(&mut self, _now: Instant, event: ClusterRemoteTrackEvent) {
        match event {
            ClusterRemoteTrackEvent::RequestKeyFrame => self.queue.push_back(Output::Event(EndpointRemoteTrackEvent::RequestKeyFrame)),
            ClusterRemoteTrackEvent::LimitBitrate { min, max } => match self.meta.control {
                Some(BitrateControlMode::MaxBitrate) => {
                    log::debug!("[EndpointRemoteTrack] dont control remote bitrate with mode is {:?}", self.meta.control);
                }
                Some(BitrateControlMode::DynamicConsumers) | None => {
                    self.cluster_bitrate_limit = Some((min, max));
                    if let Some((min, max)) = self.calc_limit_bitrate() {
                        self.queue.push_back(Output::Event(EndpointRemoteTrackEvent::LimitBitrateBps { min, max }));
                    }
                }
            },
        }
    }

    fn on_transport_event(&mut self, _now: Instant, event: RemoteTrackEvent) {
        match event {
            RemoteTrackEvent::Started { name, priority, meta: _ } => {
                self.name = Some(name.clone());
                let room = return_if_none!(self.room.as_ref());
                log::info!("[EndpointRemoteTrack] started as name {name} in room {room}");
                self.queue.push_back(Output::Cluster(*room, ClusterRemoteTrackControl::Started(TrackName(name), self.meta.clone())));
                self.queue.push_back(Output::Started(self.meta.kind, priority));
            }
            RemoteTrackEvent::Paused => {}
            RemoteTrackEvent::Resumed => {}
            RemoteTrackEvent::Media(mut media) => {
                //TODO clear self.last_layer if switched to new track
                if media.layers.is_some() {
                    log::info!("[EndpointRemoteTrack] on layers info {:?}", media.layers);
                    self.last_layers = media.layers.clone();
                }

                // We restore last_layer if key frame not contain for allow consumers fast switching
                if media.meta.is_video_key() && media.layers.is_none() {
                    log::info!("[EndpointRemoteTrack] set layers info to key-frame {:?}", media.layers);
                    media.layers = self.last_layers.clone();
                }

                let room = return_if_none!(self.room.as_ref());
                self.queue.push_back(Output::Cluster(*room, ClusterRemoteTrackControl::Media(media)));
            }
            RemoteTrackEvent::Ended => {
                let name = return_if_none!(self.name.take());
                let room = return_if_none!(self.room.as_ref());
                log::info!("[EndpointRemoteTrack] stopped with name {name} in room {room}");
                self.queue.push_back(Output::Cluster(*room, ClusterRemoteTrackControl::Ended));
                self.queue.push_back(Output::Stopped(self.meta.kind));
            }
        }
    }

    fn on_rpc_req(&mut self, _now: Instant, _req_id: EndpointReqId, _req: EndpointRemoteTrackReq) {
        todo!()
    }

    fn on_bitrate_allocation_action(&mut self, _now: Instant, action: IngressAction) {
        match action {
            IngressAction::SetBitrate(bitrate) => {
                self.allocate_bitrate = Some(bitrate);
                match self.meta.control {
                    Some(BitrateControlMode::MaxBitrate) => {
                        if let Some((min, max)) = self.calc_limit_bitrate() {
                            self.queue.push_back(Output::Event(EndpointRemoteTrackEvent::LimitBitrateBps { min, max }))
                        }
                    }
                    Some(BitrateControlMode::DynamicConsumers) | None => {}
                }
            }
        }
    }

    fn calc_limit_bitrate(&self) -> Option<(u64, u64)> {
        match (self.allocate_bitrate, self.cluster_bitrate_limit) {
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

    fn on_shutdown(&mut self, _now: Instant) {}
}

impl TaskSwitcherChild<Output> for EndpointRemoteTrack {
    type Time = Instant;
    fn pop_output(&mut self, _now: Instant) -> Option<Output> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    //TODO start in room
    //TODO start not in room
    //TODO stop in room
    //TODO stop not in room
    //TODO switched room need fire events
    //TODO send media to cluster
    //TODO handle key-frame-request feedback
    //TODO handle bitrate limit feedback
}
