//! LocalTrack take care handling client request related this track
//! It also handle feedback to source track about key-frame-request or desired-bitrate
//! Last role is rewrite media data from source track to ensure seq and timestamp is continuous even when switched to other source

use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    transport::RpcError,
};
use sans_io_runtime::Task;

use crate::{
    cluster::{ClusterLocalTrackControl, ClusterLocalTrackEvent, ClusterRoomHash},
    endpoint::{EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointLocalTrackRes, EndpointReqId},
    errors::EndpointErrors,
    transport::LocalTrackEvent,
};

pub enum Input {
    JoinRoom(ClusterRoomHash),
    LeaveRoom,
    Cluster(ClusterLocalTrackEvent),
    Event(LocalTrackEvent),
    RpcReq(EndpointReqId, EndpointLocalTrackReq),
}

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

    fn on_join_room(&mut self, now: Instant, room: ClusterRoomHash) -> Option<Output> {
        assert_eq!(self.room, None);
        assert_eq!(self.bind, None);
        log::info!("[EndpointLocalTrack] join room {room}");
        self.room = Some(room);
        None
    }

    fn on_leave_room(&mut self, _now: Instant) -> Option<Output> {
        assert_ne!(self.room, None);
        let room = self.room.take()?;
        log::info!("[EndpointLocalTrack] leave room {room}");
        let (peer, track) = self.bind.take()?;
        log::info!("[EndpointLocalTrack] leave room {room} => auto Unsubscribe {peer} {track}");
        Some(Output::Cluster(room, ClusterLocalTrackControl::Unsubscribe))
    }

    fn on_cluster_event(&mut self, _now: Instant, event: ClusterLocalTrackEvent) -> Option<Output> {
        match event {
            ClusterLocalTrackEvent::Started => todo!(),
            ClusterLocalTrackEvent::SourceChanged => {
                let room = self.room.as_ref()?;
                log::info!("[EndpointLocalTrack] source changed => request key-frame and reset seq, ts rewrite");
                Some(Output::Cluster(*room, ClusterLocalTrackControl::RequestKeyFrame))
            }
            ClusterLocalTrackEvent::Media(pkt) => Some(Output::Event(EndpointLocalTrackEvent::Media(pkt))),
            ClusterLocalTrackEvent::Ended => todo!(),
        }
    }

    fn on_transport_event(&mut self, now: Instant, event: LocalTrackEvent) -> Option<Output> {
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

    fn on_rpc_req(&mut self, now: Instant, req_id: EndpointReqId, req: EndpointLocalTrackReq) -> Option<Output> {
        match req {
            EndpointLocalTrackReq::Switch(Some((peer, track))) => {
                if let Some(room) = self.room.as_ref() {
                    log::info!("[EndpointLocalTrack] view room {room} peer {peer} track {track}");
                    self.bind = Some((peer.clone(), track.clone()));
                    self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::Subscribe(peer, track)));
                    Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Ok(()))))
                } else {
                    log::warn!("[EndpointLocalTrack] view but not in room");
                    Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Err(RpcError::new2(EndpointErrors::LocalTrackSwitchNotInRoom)))))
                }
            }
            EndpointLocalTrackReq::Switch(None) => {
                if let Some(room) = self.room.as_ref() {
                    if let Some((peer, track)) = self.bind.take() {
                        self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::Unsubscribe));
                        log::info!("[EndpointLocalTrack] unview room {room} peer {peer} track {track}");
                        Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Ok(()))))
                    } else {
                        log::warn!("[EndpointLocalTrack] unview but not bind to any source");
                        Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Err(RpcError::new2(EndpointErrors::LocalTrackSwitchNotPin)))))
                    }
                } else {
                    log::warn!("[EndpointLocalTrack] unview but not in room");
                    Some(Output::RpcRes(req_id, EndpointLocalTrackRes::Switch(Err(RpcError::new2(EndpointErrors::LocalTrackSwitchNotInRoom)))))
                }
            }
        }
    }
}

impl Task<Input, Output> for EndpointLocalTrack {
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
        self.queue.pop_front()
    }

    fn shutdown(&mut self, now: Instant) -> Option<Output> {
        None
    }
}

#[cfg(test)]
mod tests {
    //TODO view not in room
    //TODO view in room
    //TODO unview ok
    //TODO unview not ok
    //TODO room changed should fire unview
    //TODO switched source need continuous ts and seq
    //TODO should request key-frame if wait key-frame
    //TODO should forward key-frame request from transport
    //TODO local ended should unview if in viewing state
}
