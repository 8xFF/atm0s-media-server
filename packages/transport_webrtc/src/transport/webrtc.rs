use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointRemoteTrackReq, EndpointReq, EndpointReqId, EndpointRes},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportError, TransportEvent, TransportOutput, TransportState},
};
use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe},
    protobuf::{
        self,
        conn::{
            server_event::{
                room::{Event as ProtoRoomEvent2, PeerJoined, PeerLeaved, TrackStarted, TrackStopped},
                Event as ProtoServerEvent, Room as ProtoRoomEvent,
            },
            ClientEvent,
        },
        gateway::ConnectRequest,
    },
    transport::RpcError,
};
use prost::Message;
use str0m::{
    channel::{ChannelData, ChannelId},
    format::CodecConfig,
    media::{Direction, KeyframeRequestKind, MediaAdded, Mid},
    Event as Str0mEvent, IceConnectionState,
};

use crate::{media::RemoteMediaConvert, WebrtcError};

use self::{local_track::LocalTrack, remote_track::RemoteTrack};

use super::{bwe_state::BweState, InternalOutput, TransportWebrtcInternal};

const TIMEOUT_SEC: u64 = 10;

mod local_track;
mod remote_track;

#[derive(Debug)]
enum State {
    New,
    Connecting { at: Instant },
    ConnectError(TransportWebrtcError),
    Connected,
    Reconnecting { at: Instant },
    Disconnected,
}

#[derive(Debug)]
enum TransportWebrtcError {
    Timeout,
}

pub struct TransportWebrtcSdk {
    join: Option<(RoomId, PeerId, Option<String>, RoomInfoPublish, RoomInfoSubscribe)>,
    state: State,
    queue: VecDeque<InternalOutput<'static>>,
    channel: Option<ChannelId>,
    event_seq: u32,
    local_tracks: Vec<LocalTrack>,
    remote_tracks: Vec<RemoteTrack>,
    media_convert: RemoteMediaConvert,
    bwe_state: BweState,
}

impl TransportWebrtcSdk {
    pub fn new(req: ConnectRequest) -> Self {
        Self {
            join: req.join.map(|j| (j.room.into(), j.peer.into(), j.metadata, j.publish.into(), j.subscribe.into())),
            state: State::New,
            local_tracks: req.tracks.receivers.into_iter().enumerate().map(|(index, r)| LocalTrack::new((index as u16).into(), r)).collect(),
            remote_tracks: req.tracks.senders.into_iter().enumerate().map(|(index, s)| RemoteTrack::new((index as u16).into(), s)).collect(),
            queue: VecDeque::new(),
            channel: None,
            event_seq: 0,
            media_convert: RemoteMediaConvert::default(),
            bwe_state: BweState::default(),
        }
    }

    fn remote_track(&mut self, track_id: RemoteTrackId) -> Option<&mut RemoteTrack> {
        self.remote_tracks.iter_mut().find(|t| t.id() == track_id)
    }

    fn remote_track_by_mid(&mut self, mid: Mid) -> Option<&mut RemoteTrack> {
        self.remote_tracks.iter_mut().find(|t| t.mid() == Some(mid))
    }

    fn remote_track_by_name(&mut self, name: &str) -> Option<&mut RemoteTrack> {
        self.remote_tracks.iter_mut().find(|t| t.name() == name)
    }

    fn local_track(&mut self, track_id: LocalTrackId) -> Option<&mut LocalTrack> {
        self.local_tracks.iter_mut().find(|t| t.id() == track_id)
    }

    fn local_track_by_name(&mut self, name: &str) -> Option<&mut LocalTrack> {
        self.local_tracks.iter_mut().find(|t| t.name() == name)
    }

    fn build_event<'a>(&mut self, event: protobuf::conn::server_event::Event) -> Option<InternalOutput<'a>> {
        let seq = self.event_seq;
        self.event_seq += 1;
        let event = protobuf::conn::ServerEvent { seq, event: Some(event) };
        Some(InternalOutput::Str0mSendData(self.channel?, event.encode_to_vec()))
    }

    fn build_rpc_res<'a>(&mut self, req_id: u32, res: protobuf::conn::response::Response) -> Option<InternalOutput<'a>> {
        self.build_event(protobuf::conn::server_event::Event::Response(protobuf::conn::Response { req_id, response: Some(res) }))
    }

    fn build_rpc_res_err<'a>(&mut self, req_id: u32, err: RpcError) -> Option<InternalOutput<'a>> {
        let response = protobuf::conn::response::Response::Error(err.into());
        self.build_event(protobuf::conn::server_event::Event::Response(protobuf::conn::Response { req_id, response: Some(response) }))
    }
}

impl TransportWebrtcInternal for TransportWebrtcSdk {
    fn on_codec_config(&mut self, cfg: &CodecConfig) {
        self.media_convert.set_config(cfg);
    }

    fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        if let Some(init_bitrate) = self.bwe_state.on_tick(now) {
            self.queue.push_back(InternalOutput::Str0mResetBwe(init_bitrate));
        }

        match &self.state {
            State::New => {
                self.state = State::Connecting { at: now };
                return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connecting))));
            }
            State::Connecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcSdk] connect timed out after {:?} => switched to ConnectError", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::ConnectError(
                        TransportError::Timeout,
                    )))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcSdk] reconnect timed out after {:?} => switched to Disconnected", now - *at);
                    self.state = State::Disconnected;
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(Some(
                        TransportError::Timeout,
                    ))))));
                }
            }
            _ => {}
        }
        None
    }

    fn on_endpoint_event<'a>(&mut self, now: Instant, event: EndpointEvent) -> Option<InternalOutput<'a>> {
        match event {
            EndpointEvent::PeerJoined(peer, meta) => {
                log::info!("[TransportWebrtcSdk] peer {peer} joined");
                self.build_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::PeerJoined(PeerJoined {
                        peer: peer.0,
                        metadata: meta.metadata,
                    })),
                }))
            }
            EndpointEvent::PeerLeaved(peer) => {
                log::info!("[TransportWebrtcSdk] peer {peer} leaved");
                self.build_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::PeerLeaved(PeerLeaved { peer: peer.0 })),
                }))
            }
            EndpointEvent::PeerTrackStarted(peer, track, meta) => {
                log::info!("[TransportWebrtcSdk] peer {peer} track {track} started");
                self.build_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::TrackStarted(TrackStarted {
                        peer: peer.0,
                        track: track.0,
                        metadata: meta.metadata,
                    })),
                }))
            }
            EndpointEvent::PeerTrackStopped(peer, track) => {
                log::info!("[TransportWebrtcSdk] peer {peer} track {track} stopped");
                self.build_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::TrackStopped(TrackStopped { peer: peer.0, track: track.0 })),
                }))
            }
            EndpointEvent::RemoteMediaTrack(track_id, event) => match event {
                media_server_core::endpoint::EndpointRemoteTrackEvent::RequestKeyFrame => {
                    let mid = self.remote_track(track_id)?.mid()?;
                    log::info!("[TransportWebrtcSdk] request key-frame");
                    Some(InternalOutput::Str0mKeyframe(mid, KeyframeRequestKind::Fir))
                }
                media_server_core::endpoint::EndpointRemoteTrackEvent::LimitBitrateBps { min, max } => {
                    let track = self.remote_track(track_id)?;
                    let mid = track.mid()?;
                    let bitrate = track.calc_limit_bitrate(min, max);
                    log::debug!("[TransportWebrtcSdk] limit video track {mid} with bitrate {bitrate} bps");
                    Some(InternalOutput::Str0mLimitBitrate(mid, bitrate))
                }
            },
            EndpointEvent::LocalMediaTrack(track_id, event) => match event {
                EndpointLocalTrackEvent::Media(pkt) => {
                    let track = self.local_track(track_id)?;
                    let mid = track.mid()?;
                    if track.kind().is_video() {
                        self.bwe_state.on_send_video(now);
                    }
                    log::trace!("[TransportWebrtcSdk] send {:?} size {}", pkt.meta, pkt.data.len());
                    Some(InternalOutput::Str0mSendMedia(mid, pkt))
                }
                EndpointLocalTrackEvent::DesiredBitrate(_) => None,
            },
            EndpointEvent::BweConfig { current, desired } => {
                let (current, desired) = self.bwe_state.filter_bwe_config(current, desired);
                Some(InternalOutput::Str0mBwe(current, desired))
            }
            EndpointEvent::GoAway(_, _) => None,
        }
    }

    fn on_transport_rpc_res<'a>(&mut self, _now: Instant, req_id: EndpointReqId, res: EndpointRes) -> Option<InternalOutput<'a>> {
        match res {
            EndpointRes::JoinRoom(Ok(_)) => self.build_rpc_res(
                req_id.0,
                protobuf::conn::response::Response::Session(protobuf::conn::response::Session {
                    response: Some(protobuf::conn::response::session::Response::Join(protobuf::conn::response::session::RoomJoin {})),
                }),
            ),
            EndpointRes::JoinRoom(Err(err)) => self.build_rpc_res_err(req_id.0, err),
            EndpointRes::LeaveRoom(Ok(_)) => self.build_rpc_res(
                req_id.0,
                protobuf::conn::response::Response::Session(protobuf::conn::response::Session {
                    response: Some(protobuf::conn::response::session::Response::Leave(protobuf::conn::response::session::RoomLeave {})),
                }),
            ),
            EndpointRes::LeaveRoom(Err(err)) => self.build_rpc_res_err(req_id.0, err),
            EndpointRes::SubscribePeer(_) => todo!(),
            EndpointRes::UnsubscribePeer(_) => todo!(),
            EndpointRes::RemoteTrack(track_id, res) => match res {
                media_server_core::endpoint::EndpointRemoteTrackRes::Config(Ok(_)) => self.build_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Sender(protobuf::conn::response::Sender {
                        response: Some(protobuf::conn::response::sender::Response::Config(protobuf::conn::response::sender::Config {})),
                    }),
                ),
                media_server_core::endpoint::EndpointRemoteTrackRes::Config(Err(err)) => self.build_rpc_res_err(req_id.0, err),
            },
            EndpointRes::LocalTrack(_track_id, res) => match res {
                media_server_core::endpoint::EndpointLocalTrackRes::Attach(Ok(_)) => self.build_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Receiver(protobuf::conn::response::Receiver {
                        response: Some(protobuf::conn::response::receiver::Response::Attach(protobuf::conn::response::receiver::Attach {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Detach(Ok(_)) => self.build_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Receiver(protobuf::conn::response::Receiver {
                        response: Some(protobuf::conn::response::receiver::Response::Detach(protobuf::conn::response::receiver::Detach {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Config(Ok(_)) => self.build_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Receiver(protobuf::conn::response::Receiver {
                        response: Some(protobuf::conn::response::receiver::Response::Config(protobuf::conn::response::receiver::Config {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Attach(Err(err)) => self.build_rpc_res_err(req_id.0, err),
                media_server_core::endpoint::EndpointLocalTrackRes::Detach(Err(err)) => self.build_rpc_res_err(req_id.0, err),
                media_server_core::endpoint::EndpointLocalTrackRes::Config(Err(err)) => self.build_rpc_res_err(req_id.0, err),
            },
        }
    }

    fn on_str0m_event<'a>(&mut self, now: Instant, event: Str0mEvent) -> Option<InternalOutput<'a>> {
        match event {
            Str0mEvent::ChannelOpen(channel, name) => {
                self.state = State::Connected;
                self.channel = Some(channel);
                log::info!("[TransportWebrtcSdk] channel {name} opened, join state {:?}", self.join);
                if let Some((room, peer, metadata, publish, subscribe)) = &self.join {
                    self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                        0.into(),
                        EndpointReq::JoinRoom(room.clone(), peer.clone(), PeerMeta { metadata: metadata.clone() }, publish.clone(), subscribe.clone()),
                    )));
                }
                Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
            }
            Str0mEvent::ChannelData(data) => self.on_str0m_channel_data(data),
            Str0mEvent::ChannelClose(_channel) => {
                log::info!("[TransportWebrtcSdk] channel closed, leave room {:?}", self.join);
                self.state = State::Disconnected;
                Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(Some(
                    TransportError::Timeout,
                ))))))
            }
            Str0mEvent::IceConnectionStateChange(state) => self.on_str0m_state(now, state),
            Str0mEvent::MediaAdded(media) => self.on_str0m_media_added(now, media),
            Str0mEvent::RtpPacket(pkt) => {
                let mid = self.media_convert.get_mid(pkt.header.ssrc, pkt.header.ext_vals.mid)?;
                let track = self.remote_track_by_mid(mid)?.id();
                let pkt = self.media_convert.convert(pkt)?;
                log::trace!(
                    "[TransportWebrtcWhip] incoming pkt codec {:?}, seq {} ts {}, marker {}, payload {}",
                    pkt.meta,
                    pkt.seq,
                    pkt.ts,
                    pkt.marker,
                    pkt.data.len(),
                );
                Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                    track,
                    RemoteTrackEvent::Media(pkt),
                ))))
            }
            Str0mEvent::PeerStats(_stats) => None,
            Str0mEvent::MediaIngressStats(stats) => {
                log::debug!("ingress rtt {} {:?}", stats.mid, stats.rtt);
                None
            }
            Str0mEvent::MediaEgressStats(stats) => {
                log::debug!("egress rtt {} {:?}", stats.mid, stats.rtt);
                None
            }
            _ => None,
        }
    }

    fn close<'a>(&mut self, _now: Instant) -> Option<InternalOutput<'a>> {
        log::info!("[TransportWebrtcSdk] switched to disconnected with close action");
        self.state = State::Disconnected;
        Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))))
    }

    fn pop_output<'a>(&mut self, _now: Instant) -> Option<InternalOutput<'a>> {
        self.queue.pop_front()
    }
}

impl TransportWebrtcSdk {
    fn on_str0m_state<'a>(&mut self, now: Instant, state: IceConnectionState) -> Option<InternalOutput<'a>> {
        log::info!("[TransportWebrtcSdk] str0m state changed {:?}", state);

        match state {
            IceConnectionState::New => None,
            IceConnectionState::Checking => None,
            IceConnectionState::Connected | IceConnectionState::Completed => match &self.state {
                State::Reconnecting { at } => {
                    log::info!("[TransportWebrtcSdk] switched to reconnected after {:?}", now - *at);
                    self.state = State::Connected;
                    Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
                }
                _ => None,
            },
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    log::info!("[TransportWebrtcSdk] switched to reconnecting");
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Reconnecting))));
                } else {
                    return None;
                }
            }
        }
    }

    fn on_str0m_media_added<'a>(&mut self, _now: Instant, media: MediaAdded) -> Option<InternalOutput<'a>> {
        match media.direction {
            Direction::RecvOnly | Direction::SendRecv => {
                if let Some(track) = self.remote_tracks.iter_mut().find(|t| t.mid().is_none()) {
                    log::info!("[TransportWebrtcSdk] config mid {} to remote track {}", media.mid, track.name());
                    track.set_str0m(media.mid, media.simulcast.is_some());
                    Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                        track.id(),
                        RemoteTrackEvent::Started {
                            name: track.name().to_string(),
                            priority: track.priority(),
                            meta: track.meta(),
                        },
                    ))))
                } else {
                    log::warn!("[TransportWebrtcSdk] not found track for mid {}", media.mid);
                    None
                }
            }
            Direction::SendOnly => {
                if let Some(track) = self.local_tracks.iter_mut().find(|t| t.mid().is_none()) {
                    log::info!("[TransportWebrtcSdk] config mid {} to local track {}", media.mid, track.name());
                    track.set_mid(media.mid);
                    Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                        track.id(),
                        LocalTrackEvent::Started(track.kind()),
                    ))))
                } else {
                    log::warn!("[TransportWebrtcSdk] not found track for mid {}", media.mid);
                    None
                }
            }
            Direction::Inactive => {
                log::warn!("[TransportWebrtcSdk] unsupported direct Inactive");
                None
            }
        }
    }

    fn on_str0m_channel_data<'a>(&mut self, data: ChannelData) -> Option<InternalOutput<'a>> {
        let event = ClientEvent::decode(data.data.as_slice()).ok()?;
        log::info!("[TransportWebrtcSdk] on client event {:?}", event);
        match event.event? {
            protobuf::conn::client_event::Event::Request(req) => {
                let req_id = req.req_id;
                let to_out = |req: EndpointReq| -> Option<InternalOutput<'static>> { Some(InternalOutput::TransportOutput(TransportOutput::RpcReq(req_id.into(), req))) };
                match req.request? {
                    protobuf::conn::request::Request::Session(req) => match req.request? {
                        protobuf::conn::request::session::Request::Join(req) => {
                            let meta = PeerMeta { metadata: req.info.metadata };
                            to_out(EndpointReq::JoinRoom(
                                req.info.room.into(),
                                req.info.peer.into(),
                                meta,
                                req.info.publish.into(),
                                req.info.subscribe.into(),
                            ))
                        }
                        protobuf::conn::request::session::Request::Leave(_req) => to_out(EndpointReq::LeaveRoom),
                        protobuf::conn::request::session::Request::Sdp(_) => todo!(),
                        protobuf::conn::request::session::Request::Disconnect(_) => {
                            log::info!("[TransportWebrtcSdk] switched to disconnected with close action from client");
                            self.state = State::Disconnected;
                            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))))
                        }
                    },
                    protobuf::conn::request::Request::Sender(req) => {
                        let track = if let Some(track) = self.remote_track_by_name(&req.name) {
                            track
                        } else {
                            return self.build_rpc_res_err(req_id, RpcError::new2(WebrtcError::TrackNameNotFound));
                        };

                        match req.request? {
                            protobuf::conn::request::sender::Request::Attach(attach) => todo!(),
                            protobuf::conn::request::sender::Request::Detach(_) => todo!(),
                            protobuf::conn::request::sender::Request::Config(config) => to_out(EndpointReq::RemoteTrack(track.id(), EndpointRemoteTrackReq::Config(config.into()))),
                        }
                    }
                    protobuf::conn::request::Request::Receiver(req) => {
                        let track = if let Some(track) = self.local_track_by_name(&req.name) {
                            track
                        } else {
                            return self.build_rpc_res_err(req_id, RpcError::new2(WebrtcError::TrackNameNotFound));
                        };

                        match req.request? {
                            protobuf::conn::request::receiver::Request::Attach(attach) => {
                                to_out(EndpointReq::LocalTrack(track.id(), EndpointLocalTrackReq::Attach(attach.source.into(), attach.config.into())))
                            }
                            protobuf::conn::request::receiver::Request::Detach(_) => to_out(EndpointReq::LocalTrack(track.id(), EndpointLocalTrackReq::Detach())),
                            protobuf::conn::request::receiver::Request::Config(config) => to_out(EndpointReq::LocalTrack(track.id(), EndpointLocalTrackReq::Config(config.into()))),
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use media_server_core::{
        endpoint::EndpointReq,
        transport::{TransportEvent, TransportOutput, TransportState},
    };
    use media_server_protocol::{
        endpoint::{PeerMeta, RoomInfoPublish, RoomInfoSubscribe},
        protobuf::{gateway, shared},
    };
    use str0m::channel::ChannelId;

    use crate::transport::{InternalOutput, TransportWebrtcInternal};

    use super::TransportWebrtcSdk;

    fn create_channel_id() -> ChannelId {
        let mut rtc = str0m::RtcConfig::default().build();
        rtc.direct_api().create_data_channel(Default::default())
    }

    #[test]
    fn join_room_first() {
        let req = gateway::ConnectRequest {
            join: Some(shared::RoomJoin {
                room: "room".to_string(),
                peer: "peer".to_string(),
                publish: shared::RoomInfoPublish { peer: true, tracks: true },
                subscribe: shared::RoomInfoSubscribe { peers: true, tracks: true },
                metadata: Some("metadata".to_string()),
            }),
            ..Default::default()
        };

        let channel_id = create_channel_id();

        let now = Instant::now();
        let mut transport = TransportWebrtcSdk::new(req);
        assert_eq!(transport.pop_output(now), None);

        let out = transport.on_str0m_event(now, str0m::Event::ChannelOpen(channel_id, "data".to_string()));
        assert_eq!(out, Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected)))));
        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                0.into(),
                EndpointReq::JoinRoom(
                    "room".to_string().into(),
                    "peer".to_string().into(),
                    PeerMeta {
                        metadata: Some("metadata".to_string())
                    },
                    RoomInfoPublish { peer: true, tracks: true },
                    RoomInfoSubscribe { peers: true, tracks: true }
                )
            )))
        );
        assert_eq!(transport.pop_output(now), None);
    }

    #[test]
    fn join_room_lazy() {
        let req = gateway::ConnectRequest::default();

        let channel_id = create_channel_id();

        let now = Instant::now();
        let mut transport = TransportWebrtcSdk::new(req);
        assert_eq!(transport.pop_output(now), None);

        let out = transport.on_str0m_event(now, str0m::Event::ChannelOpen(channel_id, "data".to_string()));
        assert_eq!(out, Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected)))));
        assert_eq!(transport.pop_output(now), None);
    }
}
