//! LocalTrack take care handling client request related this track
//! It also handle feedback to source track about key-frame-request or desired-bitrate
//! Last role is rewrite media data from source track to ensure seq and timestamp is continuous even when switched to other source

use std::{collections::VecDeque, time::Instant};

use atm0s_sdn::TimePivot;
use media_server_protocol::{
    endpoint::{PeerId, TrackName, TrackPriority},
    media::MediaKind,
    transport::RpcError,
};
use sans_io_runtime::{return_if_none, Task, TaskSwitcherChild};

use crate::{
    cluster::{ClusterLocalTrackControl, ClusterLocalTrackEvent, ClusterRoomHash},
    endpoint::{EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointLocalTrackRes, EndpointReqId},
    errors::EndpointErrors,
    transport::LocalTrackEvent,
};

use self::packet_selector::PacketSelector;

use super::bitrate_allocator::EgressAction;

mod packet_selector;

pub enum Input {
    JoinRoom(ClusterRoomHash),
    LeaveRoom,
    Cluster(ClusterLocalTrackEvent),
    Event(LocalTrackEvent),
    RpcReq(EndpointReqId, EndpointLocalTrackReq),
    BitrateAllocation(EgressAction),
}

pub enum Output {
    Event(EndpointLocalTrackEvent),
    Cluster(ClusterRoomHash, ClusterLocalTrackControl),
    RpcRes(EndpointReqId, EndpointLocalTrackRes),
    Started(MediaKind, TrackPriority),
    Stopped(MediaKind),
}

pub struct EndpointLocalTrack {
    kind: MediaKind,
    room: Option<ClusterRoomHash>,
    bind: Option<(PeerId, TrackName)>,
    queue: VecDeque<Output>,
    selector: PacketSelector,
    timer: TimePivot,
}

impl EndpointLocalTrack {
    pub fn new(kind: MediaKind, room: Option<ClusterRoomHash>) -> Self {
        log::info!("[EndpointLocalTrack] track {kind}, room {:?}", room);
        Self {
            kind,
            room,
            bind: None,
            queue: VecDeque::new(),
            selector: PacketSelector::new(kind),
            timer: TimePivot::build(),
        }
    }

    fn on_join_room(&mut self, _now: Instant, room: ClusterRoomHash) {
        assert_eq!(self.room, None);
        assert_eq!(self.bind, None);
        log::info!("[EndpointLocalTrack] join room {room}");
        self.room = Some(room);
    }

    fn on_leave_room(&mut self, _now: Instant) {
        assert_ne!(self.room, None);
        let room = return_if_none!(self.room.take());
        log::info!("[EndpointLocalTrack] leave room {room}");
        let (peer, track) = return_if_none!(self.bind.take());
        log::info!("[EndpointLocalTrack] leave room {room} => auto Unsubscribe {peer} {track}");
        self.queue.push_back(Output::Cluster(room, ClusterLocalTrackControl::Unsubscribe));
    }

    fn on_cluster_event(&mut self, now: Instant, event: ClusterLocalTrackEvent) {
        match event {
            ClusterLocalTrackEvent::Started => todo!(),
            ClusterLocalTrackEvent::SourceChanged => {
                let room = return_if_none!(self.room.as_ref());
                log::info!("[EndpointLocalTrack] source changed => request key-frame and reset seq, ts rewrite");
                self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::RequestKeyFrame));
            }
            ClusterLocalTrackEvent::Media(channel, mut pkt) => {
                log::trace!("[EndpointLocalTrack] on media payload {:?} seq {}", pkt.meta, pkt.seq);
                let now_ms = self.timer.timestamp_ms(now);
                if self.selector.select(self.timer.timestamp_ms(now), channel, &mut pkt).is_some() {
                    self.pop_selector(now_ms);
                    self.queue.push_back(Output::Event(EndpointLocalTrackEvent::Media(pkt)));
                }
            }
            ClusterLocalTrackEvent::Ended => todo!(),
        }
    }

    fn on_transport_event(&mut self, _now: Instant, event: LocalTrackEvent) {
        log::info!("[EndpointLocalTrack] on event {:?}", event);
        match event {
            LocalTrackEvent::Started(_) => {}
            LocalTrackEvent::RequestKeyFrame => {
                let room = return_if_none!(self.room.as_ref());
                self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::RequestKeyFrame));
            }
            LocalTrackEvent::Ended => {}
        }
    }

    fn on_rpc_req(&mut self, _now: Instant, req_id: EndpointReqId, req: EndpointLocalTrackReq) {
        match req {
            EndpointLocalTrackReq::Attach(source, config) => {
                //TODO process config here
                if let Some(room) = self.room.as_ref() {
                    self.queue.push_back(Output::RpcRes(req_id, EndpointLocalTrackRes::Attach(Ok(()))));
                    let peer = source.peer;
                    let track = source.track;
                    log::info!("[EndpointLocalTrack] view room {room} peer {peer} track {track}");
                    if let Some((_peer, _track)) = self.bind.take() {
                        log::info!("[EndpointLocalTrack] view room {room} peer {peer} track {track} => unsubscribe current {_peer} {_track}");
                        self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::Unsubscribe));
                        self.queue.push_back(Output::Stopped(self.kind));
                    }
                    self.bind = Some((peer.clone(), track.clone()));
                    self.queue.push_back(Output::Started(self.kind, config.priority));
                    self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::Subscribe(peer, track)));
                    self.selector.reset();
                } else {
                    log::warn!("[EndpointLocalTrack] view but not in room");
                    self.queue
                        .push_back(Output::RpcRes(req_id, EndpointLocalTrackRes::Attach(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))));
                }
            }
            EndpointLocalTrackReq::Detach() => {
                //TODO process config here
                if let Some(room) = self.room.as_ref() {
                    if let Some((peer, track)) = self.bind.take() {
                        self.queue.push_back(Output::RpcRes(req_id, EndpointLocalTrackRes::Detach(Ok(()))));
                        self.queue.push_back(Output::Stopped(self.kind));
                        self.queue.push_back(Output::Cluster(*room, ClusterLocalTrackControl::Unsubscribe));
                        log::info!("[EndpointLocalTrack] unview room {room} peer {peer} track {track}");
                    } else {
                        log::warn!("[EndpointLocalTrack] unview but not bind to any source");
                        self.queue
                            .push_back(Output::RpcRes(req_id, EndpointLocalTrackRes::Detach(Err(RpcError::new2(EndpointErrors::LocalTrackNotPinSource)))));
                    }
                } else {
                    log::warn!("[EndpointLocalTrack] unview but not in room");
                    self.queue
                        .push_back(Output::RpcRes(req_id, EndpointLocalTrackRes::Detach(Err(RpcError::new2(EndpointErrors::EndpointNotInRoom)))));
                }
            }
            EndpointLocalTrackReq::Config(config) => {
                todo!()
            }
        }
    }

    fn on_bitrate_allocation_action(&mut self, now: Instant, action: EgressAction) {
        match action {
            EgressAction::SetBitrate(bitrate) => {
                let now_ms = self.timer.timestamp_ms(now);
                log::debug!("[EndpointLocalTrack] Limit send bitrate {bitrate}");
                self.selector.set_target_bitrate(now_ms, bitrate);
                self.pop_selector(now_ms);
                if let Some(room) = self.room {
                    self.queue.push_back(Output::Cluster(room, ClusterLocalTrackControl::DesiredBitrate(bitrate)));
                }
            }
        }
    }

    fn pop_selector(&mut self, now_ms: u64) {
        let room = if let Some(room) = self.room {
            room
        } else {
            return;
        };
        while let Some(action) = self.selector.pop_output(now_ms) {
            match action {
                packet_selector::Action::RequestKeyFrame => {
                    self.queue.push_back(Output::Cluster(room, ClusterLocalTrackControl::RequestKeyFrame));
                }
            }
        }
    }
}

impl Task<Input, Output> for EndpointLocalTrack {
    fn on_tick(&mut self, now: Instant) {
        let now_ms = self.timer.timestamp_ms(now);
        self.selector.on_tick(now_ms);
        self.pop_selector(now_ms);
    }

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

impl TaskSwitcherChild<Output> for EndpointLocalTrack {
    type Time = Instant;
    fn pop_output(&mut self, _now: Instant) -> Option<Output> {
        self.queue.pop_front()
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
