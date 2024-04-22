use std::time::Instant;

use media_server_protocol::endpoint::{TrackMeta, TrackName};

use crate::{
    cluster::{ClusterRemoteTrackControl, ClusterRemoteTrackEvent, ClusterRoomHash},
    endpoint::{EndpointRemoteTrackEvent, EndpointRemoteTrackReq, EndpointRemoteTrackRes, EndpointReqId},
    transport::RemoteTrackEvent,
};

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

    pub fn on_join_room(&mut self, now: Instant, room: ClusterRoomHash) -> Option<Output> {
        assert_eq!(self.room, None);
        self.room = Some(room);
        log::info!("[EndpointRemoteTrack] join room {room}");
        let name = self.name.clone()?;
        log::info!("[EndpointRemoteTrack] started as name {name} after join room");
        Some(Output::Cluster(room, ClusterRemoteTrackControl::Started(TrackName(name), self.meta.clone())))
    }
    pub fn on_leave_room(&mut self, now: Instant) -> Option<Output> {
        let room = self.room.take().expect("Must have room here");
        log::info!("[EndpointRemoteTrack] leave room {room}");
        let name = self.name.as_ref()?;
        log::info!("[EndpointRemoteTrack] stopped as name {name} after leave room");
        Some(Output::Cluster(room, ClusterRemoteTrackControl::Ended))
    }

    pub fn on_cluster_event(&mut self, now: Instant, event: ClusterRemoteTrackEvent) -> Option<Output> {
        match event {
            ClusterRemoteTrackEvent::RequestKeyFrame => Some(Output::Event(EndpointRemoteTrackEvent::RequestKeyFrame)),
        }
    }

    pub fn on_transport_event(&mut self, now: Instant, event: RemoteTrackEvent) -> Option<Output> {
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

    pub fn on_rpc_req(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointRemoteTrackReq) -> Option<Output> {
        None
    }

    pub fn pop_output(&mut self) -> Option<Output> {
        None
    }
}
