use std::time::{Duration, Instant};

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
    transport::{RpcError, RpcResult},
};
use prost::Message;
use sans_io_runtime::{collections::DynamicDeque, return_if_err, return_if_none};
use str0m::{
    bwe::BweKind,
    channel::{ChannelData, ChannelId},
    format::CodecConfig,
    media::{Direction, KeyframeRequestKind, MediaAdded, Mid},
    Event as Str0mEvent, IceConnectionState,
};

use crate::{media::RemoteMediaConvert, transport::InternalRpcReq, WebrtcError};

use self::{local_track::LocalTrack, remote_track::RemoteTrack};

use super::{bwe_state::BweState, InternalOutput, InternalRpcRes, TransportWebrtcInternal};

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
    queue: DynamicDeque<InternalOutput, 4>,
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
            queue: Default::default(),
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

    fn send_event(&mut self, event: protobuf::conn::server_event::Event) {
        let channel = return_if_none!(self.channel);
        let seq = self.event_seq;
        self.event_seq += 1;
        let event = protobuf::conn::ServerEvent { seq, event: Some(event) };
        self.queue.push_back(InternalOutput::Str0mSendData(channel, event.encode_to_vec()));
    }

    fn send_rpc_res(&mut self, req_id: u32, res: protobuf::conn::response::Response) {
        self.send_event(protobuf::conn::server_event::Event::Response(protobuf::conn::Response { req_id, response: Some(res) }));
    }

    fn send_rpc_res_err(&mut self, req_id: u32, err: RpcError) {
        let response = protobuf::conn::response::Response::Error(err.into());
        self.send_event(protobuf::conn::server_event::Event::Response(protobuf::conn::Response { req_id, response: Some(response) }))
    }
}

impl TransportWebrtcInternal for TransportWebrtcSdk {
    fn on_codec_config(&mut self, cfg: &CodecConfig) {
        self.media_convert.set_config(cfg);
    }

    fn on_tick(&mut self, now: Instant) {
        if let Some(init_bitrate) = self.bwe_state.on_tick(now) {
            self.queue.push_back(InternalOutput::Str0mResetBwe(init_bitrate));
        }

        match &self.state {
            State::New => {
                self.state = State::Connecting { at: now };
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connecting))));
            }
            State::Connecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcSdk] connect timed out after {:?} => switched to ConnectError", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::ConnectError(
                            TransportError::Timeout,
                        )))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcSdk] reconnect timed out after {:?} => switched to Disconnected", now - *at);
                    self.state = State::Disconnected;
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(Some(
                            TransportError::Timeout,
                        ))))));
                }
            }
            _ => {}
        }
    }

    fn on_rpc_res(&mut self, req_id: u32, res: RpcResult<InternalRpcRes>) {
        match res {
            Ok(res) => match res {
                InternalRpcRes::SetRemoteSdp(answer) => self.send_rpc_res(
                    req_id,
                    protobuf::conn::response::Response::Session(protobuf::conn::response::Session {
                        response: Some(protobuf::conn::response::session::Response::Sdp(protobuf::conn::response::session::UpdateSdp { sdp: answer })),
                    }),
                ),
            },
            Err(err) => {
                self.send_rpc_res_err(req_id, err);
            }
        }
    }

    fn on_endpoint_event(&mut self, now: Instant, event: EndpointEvent) {
        match event {
            EndpointEvent::PeerJoined(peer, meta) => {
                log::info!("[TransportWebrtcSdk] peer {peer} joined");
                self.send_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::PeerJoined(PeerJoined {
                        peer: peer.0,
                        metadata: meta.metadata,
                    })),
                }));
            }
            EndpointEvent::PeerLeaved(peer) => {
                log::info!("[TransportWebrtcSdk] peer {peer} leaved");
                self.send_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::PeerLeaved(PeerLeaved { peer: peer.0 })),
                }))
            }
            EndpointEvent::PeerTrackStarted(peer, track, meta) => {
                log::info!("[TransportWebrtcSdk] peer {peer} track {track} started");
                self.send_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::TrackStarted(TrackStarted {
                        peer: peer.0,
                        track: track.0,
                        metadata: meta.metadata,
                    })),
                }))
            }
            EndpointEvent::PeerTrackStopped(peer, track) => {
                log::info!("[TransportWebrtcSdk] peer {peer} track {track} stopped");
                self.send_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::TrackStopped(TrackStopped { peer: peer.0, track: track.0 })),
                }));
            }
            EndpointEvent::RemoteMediaTrack(track_id, event) => match event {
                media_server_core::endpoint::EndpointRemoteTrackEvent::RequestKeyFrame => {
                    let track = return_if_none!(self.remote_track(track_id));
                    let mid = return_if_none!(track.mid());
                    log::info!("[TransportWebrtcSdk] request key-frame");
                    self.queue.push_back(InternalOutput::Str0mKeyframe(mid, KeyframeRequestKind::Fir));
                }
                media_server_core::endpoint::EndpointRemoteTrackEvent::LimitBitrateBps { min, max } => {
                    let track = return_if_none!(self.remote_track(track_id));
                    let mid = return_if_none!(track.mid());
                    let bitrate = track.calc_limit_bitrate(min, max);
                    log::debug!("[TransportWebrtcSdk] limit video track {mid} with bitrate {bitrate} bps");
                    self.queue.push_back(InternalOutput::Str0mLimitBitrate(mid, bitrate));
                }
            },
            EndpointEvent::LocalMediaTrack(track_id, event) => match event {
                EndpointLocalTrackEvent::Media(pkt) => {
                    let track = return_if_none!(self.local_track(track_id));
                    let mid = return_if_none!(track.mid());
                    if track.kind().is_video() {
                        self.bwe_state.on_send_video(now);
                    }
                    log::trace!("[TransportWebrtcSdk] send {:?} size {}", pkt.meta, pkt.data.len());
                    self.queue.push_back(InternalOutput::Str0mSendMedia(mid, pkt))
                }
            },
            EndpointEvent::BweConfig { current, desired } => {
                let (current, desired) = self.bwe_state.filter_bwe_config(current, desired);
                log::debug!("[TransportWebrtcSdk] config bwe current {current} desired {desired}");
                self.queue.push_back(InternalOutput::Str0mBwe(current, desired))
            }
            EndpointEvent::GoAway(_, _) => {}
        }
    }

    fn on_transport_rpc_res(&mut self, _now: Instant, req_id: EndpointReqId, res: EndpointRes) {
        match res {
            EndpointRes::JoinRoom(Ok(_)) => self.send_rpc_res(
                req_id.0,
                protobuf::conn::response::Response::Session(protobuf::conn::response::Session {
                    response: Some(protobuf::conn::response::session::Response::Join(protobuf::conn::response::session::RoomJoin {})),
                }),
            ),
            EndpointRes::JoinRoom(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            EndpointRes::LeaveRoom(Ok(_)) => self.send_rpc_res(
                req_id.0,
                protobuf::conn::response::Response::Session(protobuf::conn::response::Session {
                    response: Some(protobuf::conn::response::session::Response::Leave(protobuf::conn::response::session::RoomLeave {})),
                }),
            ),
            EndpointRes::LeaveRoom(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            EndpointRes::SubscribePeer(_) => todo!(),
            EndpointRes::UnsubscribePeer(_) => todo!(),
            EndpointRes::RemoteTrack(_track_id, res) => match res {
                media_server_core::endpoint::EndpointRemoteTrackRes::Config(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Sender(protobuf::conn::response::Sender {
                        response: Some(protobuf::conn::response::sender::Response::Config(protobuf::conn::response::sender::Config {})),
                    }),
                ),
                media_server_core::endpoint::EndpointRemoteTrackRes::Config(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            },
            EndpointRes::LocalTrack(_track_id, res) => match res {
                media_server_core::endpoint::EndpointLocalTrackRes::Attach(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Receiver(protobuf::conn::response::Receiver {
                        response: Some(protobuf::conn::response::receiver::Response::Attach(protobuf::conn::response::receiver::Attach {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Detach(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Receiver(protobuf::conn::response::Receiver {
                        response: Some(protobuf::conn::response::receiver::Response::Detach(protobuf::conn::response::receiver::Detach {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Config(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::conn::response::Response::Receiver(protobuf::conn::response::Receiver {
                        response: Some(protobuf::conn::response::receiver::Response::Config(protobuf::conn::response::receiver::Config {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Attach(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                media_server_core::endpoint::EndpointLocalTrackRes::Detach(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                media_server_core::endpoint::EndpointLocalTrackRes::Config(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            },
        }
    }

    fn on_str0m_event(&mut self, now: Instant, event: Str0mEvent) {
        match event {
            Str0mEvent::ChannelOpen(channel, name) => {
                self.state = State::Connected;
                self.channel = Some(channel);
                log::info!("[TransportWebrtcSdk] channel {name} opened, join state {:?}", self.join);
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))));
                if let Some((room, peer, metadata, publish, subscribe)) = &self.join {
                    self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                        0.into(),
                        EndpointReq::JoinRoom(room.clone(), peer.clone(), PeerMeta { metadata: metadata.clone() }, publish.clone(), subscribe.clone()),
                    )));
                }
            }
            Str0mEvent::ChannelData(data) => self.on_str0m_channel_data(data),
            Str0mEvent::ChannelClose(_channel) => {
                log::info!("[TransportWebrtcSdk] channel closed, leave room {:?}", self.join);
                self.state = State::Disconnected;
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(Some(
                        TransportError::Timeout,
                    ))))));
            }
            Str0mEvent::IceConnectionStateChange(state) => self.on_str0m_state(now, state),
            Str0mEvent::MediaAdded(media) => self.on_str0m_media_added(now, media),
            Str0mEvent::RtpPacket(pkt) => {
                let mid = return_if_none!(self.media_convert.get_mid(pkt.header.ssrc, pkt.header.ext_vals.mid));
                let track = return_if_none!(self.remote_track_by_mid(mid)).id();
                let pkt = return_if_none!(self.media_convert.convert(pkt));
                log::trace!(
                    "[TransportWebrtcSdk] incoming pkt codec {:?}, seq {} ts {}, marker {}, payload {}",
                    pkt.meta,
                    pkt.seq,
                    pkt.ts,
                    pkt.marker,
                    pkt.data.len(),
                );
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                    track,
                    RemoteTrackEvent::Media(pkt),
                ))));
            }
            Str0mEvent::EgressBitrateEstimate(BweKind::Remb(_, bitrate)) | Str0mEvent::EgressBitrateEstimate(BweKind::Twcc(bitrate)) => {
                let bitrate2 = self.bwe_state.filter_bwe(bitrate.as_u64());
                log::debug!("[TransportWebrtcSdk] on rewrite bwe {bitrate} => {bitrate2} bps");
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::EgressBitrateEstimate(bitrate2))));
            }
            Str0mEvent::PeerStats(_stats) => {}
            Str0mEvent::MediaIngressStats(stats) => {
                log::debug!("ingress rtt {} {:?}", stats.mid, stats.rtt);
            }
            Str0mEvent::MediaEgressStats(stats) => {
                log::debug!("egress rtt {} {:?}", stats.mid, stats.rtt);
            }
            _ => {}
        }
    }

    fn close(&mut self, _now: Instant) {
        log::info!("[TransportWebrtcSdk] switched to disconnected with close action");
        self.state = State::Disconnected;
        self.queue
            .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))))
    }

    fn pop_output(&mut self, _now: Instant) -> Option<InternalOutput> {
        self.queue.pop_front()
    }
}

impl TransportWebrtcSdk {
    fn on_str0m_state(&mut self, now: Instant, state: IceConnectionState) {
        log::info!("[TransportWebrtcSdk] str0m state changed {:?}", state);

        match state {
            IceConnectionState::New => {}
            IceConnectionState::Checking => {}
            IceConnectionState::Connected | IceConnectionState::Completed => match &self.state {
                State::Reconnecting { at } => {
                    log::info!("[TransportWebrtcSdk] switched to reconnected after {:?}", now - *at);
                    self.state = State::Connected;
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
                }
                _ => {}
            },
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    log::info!("[TransportWebrtcSdk] switched to reconnecting");
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Reconnecting))));
                }
            }
        }
    }

    fn on_str0m_media_added(&mut self, _now: Instant, media: MediaAdded) {
        match media.direction {
            Direction::RecvOnly | Direction::SendRecv => {
                if let Some(track) = self.remote_tracks.iter_mut().find(|t| t.mid().is_none()) {
                    log::info!("[TransportWebrtcSdk] config mid {} to remote track {}", media.mid, track.name());
                    track.set_str0m(media.mid, media.simulcast.is_some());
                    if track.has_source() {
                        self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                            track.id(),
                            RemoteTrackEvent::Started {
                                name: track.name().to_string(),
                                priority: track.priority(),
                                meta: track.meta(),
                            },
                        ))));
                    } else {
                        log::info!("[TransportWebrtcSdk] remote track without source => in waiting state");
                    }
                } else {
                    log::warn!("[TransportWebrtcSdk] not found track for mid {}", media.mid);
                }
            }
            Direction::SendOnly => {
                if let Some(track) = self.local_tracks.iter_mut().find(|t| t.mid().is_none()) {
                    log::info!("[TransportWebrtcSdk] config mid {} to local track {}", media.mid, track.name());
                    track.set_mid(media.mid);
                    self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                        track.id(),
                        LocalTrackEvent::Started(track.kind()),
                    ))))
                } else {
                    log::warn!("[TransportWebrtcSdk] not found track for mid {}", media.mid);
                }
            }
            Direction::Inactive => {
                log::warn!("[TransportWebrtcSdk] unsupported direct Inactive");
            }
        }
    }

    fn on_str0m_channel_data(&mut self, data: ChannelData) {
        let event = return_if_err!(ClientEvent::decode(data.data.as_slice()));
        log::info!("[TransportWebrtcSdk] on client event {:?}", event);
        match return_if_none!(event.event) {
            protobuf::conn::client_event::Event::Request(req) => {
                let req_id = req.req_id;
                let build_req = |req: EndpointReq| InternalOutput::TransportOutput(TransportOutput::RpcReq(req_id.into(), req));
                match return_if_none!(req.request) {
                    protobuf::conn::request::Request::Session(req) => match return_if_none!(req.request) {
                        protobuf::conn::request::session::Request::Join(req) => {
                            let meta = PeerMeta { metadata: req.info.metadata };
                            self.queue.push_back(build_req(EndpointReq::JoinRoom(
                                req.info.room.into(),
                                req.info.peer.into(),
                                meta,
                                req.info.publish.into(),
                                req.info.subscribe.into(),
                            )));
                        }
                        protobuf::conn::request::session::Request::Leave(_req) => self.queue.push_back(build_req(EndpointReq::LeaveRoom)),
                        protobuf::conn::request::session::Request::Sdp(req) => {
                            for (index, s) in req.tracks.senders.into_iter().enumerate() {
                                if self.remote_track_by_name(&s.name).is_none() {
                                    log::info!("[TransportWebrtcSdk] added new remote track {:?}", s);
                                    self.remote_tracks.push(RemoteTrack::new((index as u16).into(), s));
                                }
                            }

                            for (index, r) in req.tracks.receivers.into_iter().enumerate() {
                                if self.local_track_by_name(&r.name).is_none() {
                                    log::info!("[TransportWebrtcSdk] added new local track {:?}", r);
                                    self.local_tracks.push(LocalTrack::new((index as u16).into(), r));
                                }
                            }
                            self.queue.push_back(InternalOutput::RpcReq(req_id, InternalRpcReq::SetRemoteSdp(req.sdp)));
                        }
                        protobuf::conn::request::session::Request::Disconnect(_) => {
                            log::info!("[TransportWebrtcSdk] switched to disconnected with close action from client");
                            self.state = State::Disconnected;
                            self.queue
                                .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))))
                        }
                    },
                    protobuf::conn::request::Request::Sender(req) => {
                        let track = if let Some(track) = self.remote_track_by_name(&req.name) {
                            track
                        } else {
                            log::warn!("[TransportWebrtcSdk] request from unknown sender {}", req.name);
                            return self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::TrackNameNotFound));
                        };
                        let track_id = track.id();

                        match return_if_none!(req.request) {
                            protobuf::conn::request::sender::Request::Attach(attach) => {
                                if !track.has_source() {
                                    track.set_source(attach.source);
                                    let event = InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                                        track_id,
                                        RemoteTrackEvent::Started {
                                            name: track.name().to_string(),
                                            priority: track.priority(),
                                            meta: track.meta(),
                                        },
                                    )));
                                    self.send_rpc_res(
                                        req_id,
                                        protobuf::conn::response::Response::Sender(protobuf::conn::response::Sender {
                                            response: Some(protobuf::conn::response::sender::Response::Attach(protobuf::conn::response::sender::Attach {})),
                                        }),
                                    );
                                    self.queue.push_back(event);
                                } else {
                                    self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::TrackAlreadyAttached));
                                }
                            }
                            protobuf::conn::request::sender::Request::Detach(_) => {
                                if track.has_source() {
                                    track.del_source();
                                    self.send_rpc_res(
                                        req_id,
                                        protobuf::conn::response::Response::Sender(protobuf::conn::response::Sender {
                                            response: Some(protobuf::conn::response::sender::Response::Detach(protobuf::conn::response::sender::Detach {})),
                                        }),
                                    );
                                } else {
                                    self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::TrackNameNotFound));
                                }
                            }
                            protobuf::conn::request::sender::Request::Config(config) => {
                                self.queue.push_back(build_req(EndpointReq::RemoteTrack(track_id, EndpointRemoteTrackReq::Config(config.into()))))
                            }
                        }
                    }
                    protobuf::conn::request::Request::Receiver(req) => {
                        let track = if let Some(track) = self.local_track_by_name(&req.name) {
                            track
                        } else {
                            log::warn!("[TransportWebrtcSdk] request from unknown receiver {}", req.name);
                            return self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::TrackNameNotFound));
                        };
                        let track_id = track.id();

                        match return_if_none!(req.request) {
                            protobuf::conn::request::receiver::Request::Attach(attach) => self
                                .queue
                                .push_back(build_req(EndpointReq::LocalTrack(track_id, EndpointLocalTrackReq::Attach(attach.source.into(), attach.config.into())))),
                            protobuf::conn::request::receiver::Request::Detach(_) => self.queue.push_back(build_req(EndpointReq::LocalTrack(track_id, EndpointLocalTrackReq::Detach()))),
                            protobuf::conn::request::receiver::Request::Config(config) => {
                                self.queue.push_back(build_req(EndpointReq::LocalTrack(track_id, EndpointLocalTrackReq::Config(config.into()))))
                            }
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

        transport.on_str0m_event(now, str0m::Event::ChannelOpen(channel_id, "data".to_string()));
        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
        );
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

        transport.on_str0m_event(now, str0m::Event::ChannelOpen(channel_id, "data".to_string()));
        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
        );
        assert_eq!(transport.pop_output(now), None);
    }
}
