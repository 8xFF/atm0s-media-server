use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    transport::RpcError,
};

use crate::{
    cluster::{ClusterLocalTrackControl, ClusterLocalTrackEvent, ClusterRoomHash},
    endpoint::{EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointLocalTrackRes, EndpointReqId},
    errors::EndpointErrors,
    transport::LocalTrackEvent,
};

pub enum Output {
    Event(EndpointLocalTrackEvent),
    Cluster(ClusterRoomHash, ClusterLocalTrackControl),
    RpcRes(EndpointReqId, EndpointLocalTrackRes),
}

pub struct EndpointLocalTrack {
    room: Option<ClusterRoomHash>,
    bind: Option<(PeerId, TrackName)>,
    queue: VecDeque<Output>,
}

impl EndpointLocalTrack {
    pub fn new(room: Option<ClusterRoomHash>) -> Self {
        Self {
            room,
            bind: None,
            queue: VecDeque::new(),
        }
    }

    pub fn on_join_room(&mut self, now: Instant, room: ClusterRoomHash) -> Option<Output> {
        assert_eq!(self.room, None);
        assert_eq!(self.bind, None);
        log::info!("[EndpointLocalTrack] join room {room}");
        self.room = Some(room);
        None
    }

    pub fn on_leave_room(&mut self, now: Instant) -> Option<Output> {
        assert_ne!(self.room, None);
        let room = self.room.take()?;
        log::info!("[EndpointLocalTrack] leave room {room}");
        self.bind.take()?;
        None
    }

    pub fn on_cluster_event(&mut self, now: Instant, event: ClusterLocalTrackEvent) -> Option<Output> {
        match event {
            ClusterLocalTrackEvent::Started => todo!(),
            ClusterLocalTrackEvent::Media(pkt) => Some(Output::Event(EndpointLocalTrackEvent::Media(pkt))),
            ClusterLocalTrackEvent::Ended => todo!(),
        }
    }

    pub fn on_transport_event(&mut self, now: Instant, event: LocalTrackEvent) -> Option<Output> {
        log::info!("[EndpointLocalTrack] on event {:?}", event);
        match event {
            LocalTrackEvent::Started => None,
            LocalTrackEvent::RequestKeyFrame => {
                let room = self.room.as_ref()?;
                Some(Output::Cluster(*room, ClusterLocalTrackControl::RequestKeyFrame))
            }
            LocalTrackEvent::Ended => None,
        }
    }

    pub fn on_rpc_req(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointLocalTrackReq) -> Option<Output> {
        match req {
            EndpointLocalTrackReq::Switch(Some((peer, track))) => {
                if let Some(room) = self.room.as_ref() {
                    log::info!("[EndpointLocalTrack] view room {room} peer {peer} track {track}");
                    self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::Subscribe(peer, track)));
                    Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Ok(()))))
                } else {
                    log::warn!("[EndpointLocalTrack] view but not in room");
                    Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Err(RpcError::new2(EndpointErrors::LocalTrackSwitchNotInRoom)))))
                }
            }
            EndpointLocalTrackReq::Switch(None) => {
                if let Some(room) = self.room.as_ref() {
                    self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::Unsubscribe));
                    log::info!("[EndpointLocalTrack] unview room {room}");
                    Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Ok(()))))
                } else {
                    log::warn!("[EndpointLocalTrack] unview but not in room");
                    Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Err(RpcError::new2(EndpointErrors::LocalTrackSwitchNotInRoom)))))
                }
            }
        }
    }

    pub fn pop_output(&mut self) -> Option<Output> {
        self.queue.pop_front()
    }
}
