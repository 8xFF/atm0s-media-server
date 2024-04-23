//! RemoteTrack take care about publish local media to sdn, and react with feedback from consumers

use std::time::Instant;

use media_server_protocol::endpoint::{TrackMeta, TrackName};
use sans_io_runtime::Task;

use crate::{
    cluster::{ClusterRemoteTrackControl, ClusterRemoteTrackEvent, ClusterRoomHash},
    endpoint::{EndpointRemoteTrackEvent, EndpointRemoteTrackReq, EndpointRemoteTrackRes, EndpointReqId},
    transport::RemoteTrackEvent,
};

pub enum Input {
    JoinRoom(ClusterRoomHash),
    LeaveRoom,
    Cluster(ClusterRemoteTrackEvent),
    Event(RemoteTrackEvent),
    RpcReq(EndpointReqId, EndpointRemoteTrackReq),
}

pub enum Output {
    Event(EndpointRemoteTrackEvent),
    Cluster(ClusterRoomHash, ClusterRemoteTrackControl),
    RpcRes(EndpointReqId, EndpointRemoteTrackRes),
}

pub struct EndpointRemoteTrack {
    meta: TrackMeta,
    room: Option<ClusterRoomHash>,
    name: Option<String>,
}

impl EndpointRemoteTrack {
    pub fn new(room: Option<ClusterRoomHash>, meta: TrackMeta) -> Self {
        Self { meta, room, name: None }
    }

    fn on_join_room(&mut self, now: Instant, room: ClusterRoomHash) -> Option<Output> {
        assert_eq!(self.room, None);
        self.room = Some(room);
        log::info!("[EndpointRemoteTrack] join room {room}");
        let name = self.name.clone()?;
        log::info!("[EndpointRemoteTrack] started as name {name} after join room");
        Some(Output::Cluster(room, ClusterRemoteTrackControl::Started(TrackName(name), self.meta.clone())))
    }
    fn on_leave_room(&mut self, now: Instant) -> Option<Output> {
        let room = self.room.take().expect("Must have room here");
        log::info!("[EndpointRemoteTrack] leave room {room}");
        let name = self.name.as_ref()?;
        log::info!("[EndpointRemoteTrack] stopped as name {name} after leave room");
        Some(Output::Cluster(room, ClusterRemoteTrackControl::Ended))
    }

    fn on_cluster_event(&mut self, now: Instant, event: ClusterRemoteTrackEvent) -> Option<Output> {
        match event {
            ClusterRemoteTrackEvent::RequestKeyFrame => Some(Output::Event(EndpointRemoteTrackEvent::RequestKeyFrame)),
            ClusterRemoteTrackEvent::LimitBitrate { min, max } => {
                //TODO based on scaling type
                Some(Output::Event(EndpointRemoteTrackEvent::LimitBitrateBps(min as u64)))
            }
        }
    }

    fn on_transport_event(&mut self, now: Instant, event: RemoteTrackEvent) -> Option<Output> {
        match event {
            RemoteTrackEvent::Started { name, meta: _ } => {
                self.name = Some(name.clone());
                let room = self.room.as_ref()?;
                log::info!("[EndpointRemoteTrack] started as name {name}");
                Some(Output::Cluster(*room, ClusterRemoteTrackControl::Started(TrackName(name), self.meta.clone())))
            }
            RemoteTrackEvent::Paused => None,
            RemoteTrackEvent::Resumed => None,
            RemoteTrackEvent::Media(media) => {
                let room = self.room.as_ref()?;
                Some(Output::Cluster(*room, ClusterRemoteTrackControl::Media(media)))
            }
            RemoteTrackEvent::Ended => {
                let name = self.name.take()?;
                let room = self.room.as_ref()?;
                log::info!("[EndpointRemoteTrack] stopped with name {name}");
                Some(Output::Cluster(*room, ClusterRemoteTrackControl::Ended))
            }
        }
    }

    fn on_rpc_req(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq) -> Option<Output> {
        None
    }
}

impl Task<Input, Output> for EndpointRemoteTrack {
    fn on_tick(&mut self, now: Instant) -> Option<Output> {
        None
    }

    fn on_event(&mut self, now: Instant, input: Input) -> Option<Output> {
        match input {
            Input::JoinRoom(room) => self.on_join_room(now, room),
            Input::LeaveRoom => self.on_leave_room(now),
            Input::Cluster(event) => self.on_cluster_event(now, event),
            Input::Event(event) => self.on_transport_event(now, event),
            Input::RpcReq(req_id, req) => self.on_rpc_req(now, req_id, req),
        }
    }

    fn pop_output(&mut self, now: Instant) -> Option<Output> {
        None
    }

    fn shutdown(&mut self, now: Instant) -> Option<Output> {
        None
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
