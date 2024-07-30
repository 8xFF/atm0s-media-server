use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::{
        EndpointAudioMixerReq, EndpointEvent, EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointMessageChannelReq, EndpointMessageChannelRes, EndpointRemoteTrackReq, EndpointReq, EndpointReqId,
        EndpointRes,
    },
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportError, TransportEvent, TransportOutput, TransportState},
};
use media_server_protocol::{
    endpoint::{AudioMixerConfig, PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe},
    protobuf::{
        self,
        features::{
            mixer::{
                server_event::{Event as ProtoFeatureMixerEvent2, SlotSet, SlotUnset},
                ServerEvent as ProtoFeatureMixerEvent,
            },
            server_event::Event as ProtoFeaturesEvent2,
            ServerEvent as ProtoFeaturesEvent,
        },
        gateway::ConnectRequest,
        session::{
            request::room::channel_control as RoomChannelControlReq,
            response::{
                room::{
                    channel_control::{Control, Publish, StartPublish, StopPublish, Subscribe, Unsubscribe},
                    ChannelControl, Response as ProtoRoomSessionResponses,
                },
                Room as ProtoRoomSessionResponse,
            },
            server_event::{
                receiver::{Event as ProtoReceiverEvent, State as ProtoReceiverState, VoiceActivity as ProtoReceiverVoiceActivity},
                room::{ChannelMessage, Event as ProtoRoomEvent2, PeerJoined, PeerLeaved, TrackStarted, TrackStopped},
                sender::{Event as ProtoSenderEvent, State as ProtoSenderState},
                Event as ProtoServerEvent, Receiver as ProtoReceiverEventContainer, Room as ProtoRoomEvent, Sender as ProtoSenderEventContainer,
            },
            ClientEvent,
        },
        shared::{receiver::Source as ProtoReceiverSource, sender::Status as ProtoSenderStatus, Kind},
    },
    tokens::WebrtcToken,
    transport::{RpcError, RpcResult},
};
use media_server_secure::MediaEdgeSecure;
use prost::Message;
use sans_io_runtime::{collections::DynamicDeque, return_if_err, return_if_none};
use str0m::{
    bwe::BweKind,
    channel::ChannelId,
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

pub struct TransportWebrtcSdk<ES> {
    remote: IpAddr,
    join: Option<(RoomId, PeerId, Option<String>, RoomInfoPublish, RoomInfoSubscribe)>,
    state: State,
    queue: DynamicDeque<InternalOutput, 4>,
    channel: Option<ChannelId>,
    event_seq: u32,
    local_tracks: Vec<LocalTrack>,
    remote_tracks: Vec<RemoteTrack>,
    audio_mixer: Option<AudioMixerConfig>,
    media_convert: RemoteMediaConvert,
    bwe_state: BweState,
    secure: Arc<ES>,
}

impl<ES> TransportWebrtcSdk<ES> {
    pub fn new(req: ConnectRequest, secure: Arc<ES>, remote: IpAddr) -> Self {
        let tracks = req.tracks.unwrap_or_default();
        let local_tracks: Vec<LocalTrack> = tracks.receivers.into_iter().enumerate().map(|(index, r)| LocalTrack::new((index as u16).into(), r)).collect();
        let remote_tracks: Vec<RemoteTrack> = tracks.senders.into_iter().enumerate().map(|(index, s)| RemoteTrack::new((index as u16).into(), s)).collect();
        if let Some(j) = req.join {
            Self {
                remote,
                join: Some((j.room.into(), j.peer.into(), j.metadata, j.publish.unwrap_or_default().into(), j.subscribe.unwrap_or_default().into())),
                state: State::New,
                audio_mixer: j.features.and_then(|f| {
                    f.mixer.and_then(|m| {
                        Some(AudioMixerConfig {
                            mode: m.mode().into(),
                            outputs: m
                                .outputs
                                .iter()
                                .map(|r| local_tracks.iter().find(|l| l.name() == r.as_str()).map(|l| l.id()))
                                .flatten()
                                .collect::<Vec<_>>(),
                            sources: m.sources.into_iter().map(|s| s.into()).collect::<Vec<_>>(),
                        })
                    })
                }),
                local_tracks,
                remote_tracks,
                queue: Default::default(),
                channel: None,
                event_seq: 0,
                media_convert: RemoteMediaConvert::default(),
                bwe_state: BweState::default(),
                secure,
            }
        } else {
            Self {
                remote,
                join: None,
                state: State::New,
                local_tracks,
                remote_tracks,
                audio_mixer: None,
                queue: Default::default(),
                channel: None,
                event_seq: 0,
                media_convert: RemoteMediaConvert::default(),
                bwe_state: BweState::default(),
                secure,
            }
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

    fn local_track_by_mid(&mut self, mid: Mid) -> Option<&mut LocalTrack> {
        self.local_tracks.iter_mut().find(|t| t.mid() == Some(mid))
    }

    fn local_track_by_name(&mut self, name: &str) -> Option<&mut LocalTrack> {
        self.local_tracks.iter_mut().find(|t| t.name() == name)
    }

    fn send_event(&mut self, event: protobuf::session::server_event::Event) {
        let channel = return_if_none!(self.channel);
        let seq = self.event_seq;
        self.event_seq += 1;
        let event = protobuf::session::ServerEvent { seq, event: Some(event) };
        self.queue.push_back(InternalOutput::Str0mSendData(channel, event.encode_to_vec()));
    }

    fn send_rpc_res(&mut self, req_id: u32, res: protobuf::session::response::Response) {
        self.send_event(protobuf::session::server_event::Event::Response(protobuf::session::Response { req_id, response: Some(res) }));
    }

    fn send_rpc_res_err(&mut self, req_id: u32, err: RpcError) {
        let response = protobuf::session::response::Response::Error(err.into());
        self.send_event(protobuf::session::server_event::Event::Response(protobuf::session::Response { req_id, response: Some(response) }))
    }
}

impl<ES: MediaEdgeSecure> TransportWebrtcInternal for TransportWebrtcSdk<ES> {
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
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connecting(self.remote)))));
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
                    protobuf::session::response::Response::Session(protobuf::session::response::Session {
                        response: Some(protobuf::session::response::session::Response::Sdp(protobuf::session::response::session::UpdateSdp { sdp: answer })),
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
            EndpointEvent::PeerLeaved(peer, _meta) => {
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
                        kind: Kind::from(meta.kind) as i32,
                        metadata: meta.metadata,
                    })),
                }))
            }
            EndpointEvent::PeerTrackStopped(peer, track, meta) => {
                log::info!("[TransportWebrtcSdk] peer {peer} track {track} stopped");
                self.send_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::TrackStopped(TrackStopped {
                        peer: peer.0,
                        track: track.0,
                        kind: Kind::from(meta.kind) as i32,
                    })),
                }));
            }
            EndpointEvent::AudioMixer(event) => match event {
                media_server_core::endpoint::EndpointAudioMixerEvent::SlotSet(slot, peer, track) => {
                    log::info!("[TransportWebrtcSdk] audio mixer slot {slot} set to {peer}/{track}");
                    self.send_event(ProtoServerEvent::Features(ProtoFeaturesEvent {
                        event: Some(ProtoFeaturesEvent2::Mixer(ProtoFeatureMixerEvent {
                            event: Some(ProtoFeatureMixerEvent2::SlotSet(SlotSet {
                                slot: slot as u32,
                                source: Some(ProtoReceiverSource { peer: peer.0, track: track.0 }),
                            })),
                        })),
                    }))
                }
                media_server_core::endpoint::EndpointAudioMixerEvent::SlotUnset(slot) => {
                    log::info!("[TransportWebrtcSdk] audio mixer slot {slot} unset");
                    self.send_event(ProtoServerEvent::Features(ProtoFeaturesEvent {
                        event: Some(ProtoFeaturesEvent2::Mixer(ProtoFeatureMixerEvent {
                            event: Some(ProtoFeatureMixerEvent2::SlotUnset(SlotUnset { slot: slot as u32 })),
                        })),
                    }))
                }
            },
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
                EndpointLocalTrackEvent::Status(status) => {
                    let track = return_if_none!(self.local_track(track_id)).name().to_string();
                    log::info!("[TransportWebrtcSdk] track {track} set status {:?}", status);
                    self.send_event(ProtoServerEvent::Receiver(ProtoReceiverEventContainer {
                        name: track,
                        event: Some(ProtoReceiverEvent::State(ProtoReceiverState { status: status as i32 })),
                    }));
                }
                EndpointLocalTrackEvent::VoiceActivity(level) => {
                    let track = return_if_none!(self.local_track(track_id)).name().to_string();
                    log::info!("[TransportWebrtcSdk] track {track} set audio_level {:?}", level);
                    self.send_event(ProtoServerEvent::Receiver(ProtoReceiverEventContainer {
                        name: track,
                        event: Some(ProtoReceiverEvent::VoiceActivity(ProtoReceiverVoiceActivity { audio_level: level as i32 })),
                    }));
                }
            },
            EndpointEvent::BweConfig { current, desired } => {
                let (current, desired) = self.bwe_state.filter_bwe_config(current, desired);
                log::debug!("[TransportWebrtcSdk] config bwe current {current} desired {desired}");
                self.queue.push_back(InternalOutput::Str0mBwe(current, desired))
            }
            EndpointEvent::ChannelMessage(label, from, message) => {
                log::info!("[TransportWebrtcSdk] datachannel message {label}");
                self.send_event(ProtoServerEvent::Room(ProtoRoomEvent {
                    event: Some(ProtoRoomEvent2::ChannelMessage(ChannelMessage { label, peer: from.0, message })),
                }))
            }
            EndpointEvent::GoAway(_, _) => {}
        }
    }

    fn on_transport_rpc_res(&mut self, _now: Instant, req_id: EndpointReqId, res: EndpointRes) {
        match res {
            EndpointRes::JoinRoom(Ok(_)) => self.send_rpc_res(
                req_id.0,
                protobuf::session::response::Response::Session(protobuf::session::response::Session {
                    response: Some(protobuf::session::response::session::Response::Join(protobuf::session::response::session::Join {})),
                }),
            ),
            EndpointRes::JoinRoom(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            EndpointRes::LeaveRoom(Ok(_)) => self.send_rpc_res(
                req_id.0,
                protobuf::session::response::Response::Session(protobuf::session::response::Session {
                    response: Some(protobuf::session::response::session::Response::Leave(protobuf::session::response::session::Leave {})),
                }),
            ),
            EndpointRes::LeaveRoom(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            EndpointRes::SubscribePeer(_) => todo!(),
            EndpointRes::UnsubscribePeer(_) => todo!(),
            EndpointRes::RemoteTrack(_track_id, res) => match res {
                media_server_core::endpoint::EndpointRemoteTrackRes::Config(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::session::response::Response::Sender(protobuf::session::response::Sender {
                        response: Some(protobuf::session::response::sender::Response::Config(protobuf::session::response::sender::Config {})),
                    }),
                ),
                media_server_core::endpoint::EndpointRemoteTrackRes::Config(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            },
            EndpointRes::LocalTrack(_track_id, res) => match res {
                media_server_core::endpoint::EndpointLocalTrackRes::Attach(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::session::response::Response::Receiver(protobuf::session::response::Receiver {
                        response: Some(protobuf::session::response::receiver::Response::Attach(protobuf::session::response::receiver::Attach {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Detach(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::session::response::Response::Receiver(protobuf::session::response::Receiver {
                        response: Some(protobuf::session::response::receiver::Response::Detach(protobuf::session::response::receiver::Detach {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Config(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::session::response::Response::Receiver(protobuf::session::response::Receiver {
                        response: Some(protobuf::session::response::receiver::Response::Config(protobuf::session::response::receiver::Config {})),
                    }),
                ),
                media_server_core::endpoint::EndpointLocalTrackRes::Attach(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                media_server_core::endpoint::EndpointLocalTrackRes::Detach(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                media_server_core::endpoint::EndpointLocalTrackRes::Config(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            },
            EndpointRes::AudioMixer(res) => match res {
                media_server_core::endpoint::EndpointAudioMixerRes::Attach(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::session::response::Response::Features(protobuf::features::Response {
                        response: Some(protobuf::features::response::Response::Mixer(protobuf::features::mixer::Response {
                            response: Some(protobuf::features::mixer::response::Response::Attach(protobuf::features::mixer::response::Attach {})),
                        })),
                    }),
                ),
                media_server_core::endpoint::EndpointAudioMixerRes::Detach(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    protobuf::session::response::Response::Features(protobuf::features::Response {
                        response: Some(protobuf::features::response::Response::Mixer(protobuf::features::mixer::Response {
                            response: Some(protobuf::features::mixer::response::Response::Detach(protobuf::features::mixer::response::Detach {})),
                        })),
                    }),
                ),
                media_server_core::endpoint::EndpointAudioMixerRes::Attach(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                media_server_core::endpoint::EndpointAudioMixerRes::Detach(Err(err)) => self.send_rpc_res_err(req_id.0, err),
            },
            EndpointRes::MessageChannel(label, control) => match control {
                EndpointMessageChannelRes::Subscribe(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    media_server_protocol::protobuf::session::response::Response::Room(ProtoRoomSessionResponse {
                        response: Some(ProtoRoomSessionResponses::ChannelControl(ChannelControl {
                            label,
                            control: Some(Control::Sub(Subscribe {})),
                        })),
                    }),
                ),

                EndpointMessageChannelRes::Unsubscribe(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    media_server_protocol::protobuf::session::response::Response::Room(ProtoRoomSessionResponse {
                        response: Some(ProtoRoomSessionResponses::ChannelControl(ChannelControl {
                            label,
                            control: Some(Control::Unsub(Unsubscribe {})),
                        })),
                    }),
                ),
                EndpointMessageChannelRes::StartPublish(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    media_server_protocol::protobuf::session::response::Response::Room(ProtoRoomSessionResponse {
                        response: Some(ProtoRoomSessionResponses::ChannelControl(ChannelControl {
                            label,
                            control: Some(Control::StartPub(StartPublish {})),
                        })),
                    }),
                ),
                EndpointMessageChannelRes::StopPublish(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    media_server_protocol::protobuf::session::response::Response::Room(ProtoRoomSessionResponse {
                        response: Some(ProtoRoomSessionResponses::ChannelControl(ChannelControl {
                            label,
                            control: Some(Control::StopPub(StopPublish {})),
                        })),
                    }),
                ),
                EndpointMessageChannelRes::PublishData(Ok(_)) => self.send_rpc_res(
                    req_id.0,
                    media_server_protocol::protobuf::session::response::Response::Room(ProtoRoomSessionResponse {
                        response: Some(ProtoRoomSessionResponses::ChannelControl(ChannelControl {
                            label,
                            control: Some(Control::Pub(Publish {})),
                        })),
                    }),
                ),
                EndpointMessageChannelRes::Subscribe(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                EndpointMessageChannelRes::Unsubscribe(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                EndpointMessageChannelRes::StartPublish(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                EndpointMessageChannelRes::StopPublish(Err(err)) => self.send_rpc_res_err(req_id.0, err),
                EndpointMessageChannelRes::PublishData(Err(err)) => self.send_rpc_res_err(req_id.0, err),
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
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected(self.remote))))); //TODO get paired ip from webrtc
                if let Some((room, peer, metadata, publish, subscribe)) = &self.join {
                    self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                        0.into(),
                        EndpointReq::JoinRoom(
                            room.clone(),
                            peer.clone(),
                            PeerMeta { metadata: metadata.clone() },
                            publish.clone(),
                            subscribe.clone(),
                            self.audio_mixer.take(),
                        ),
                    )));
                }
            }
            Str0mEvent::ChannelData(data) => {
                let event = return_if_err!(ClientEvent::decode(data.data.as_slice()));
                self.on_str0m_channel_event(event);
            }
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
            Str0mEvent::KeyframeRequest(req) => {
                log::info!("[TransportWebrtcSdk] request key-frame");
                let track = return_if_none!(self.local_track_by_mid(req.mid)).id();
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                    track,
                    LocalTrackEvent::RequestKeyFrame,
                ))));
            }
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
            Str0mEvent::StreamPaused(event) => {
                // We need to map media ssrc here for avoiding unknown pkt
                // without it, sometime we will failed to restore session from restart-ice
                self.media_convert.get_mid(event.ssrc, Some(event.mid));

                let track = return_if_none!(self.remote_track_by_mid(event.mid)).name().to_string();
                let status = if event.paused {
                    ProtoSenderStatus::Inactive
                } else {
                    ProtoSenderStatus::Active
                };

                log::info!("[TransportWebrtcSdk] track {track} mid {} ssrc {} set status {:?}", event.mid, event.ssrc, status);
                self.send_event(ProtoServerEvent::Sender(ProtoSenderEventContainer {
                    name: track,
                    event: Some(ProtoSenderEvent::State(ProtoSenderState { status: status as i32 })),
                }));
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

impl<ES: MediaEdgeSecure> TransportWebrtcSdk<ES> {
    fn on_str0m_state(&mut self, now: Instant, state: IceConnectionState) {
        log::info!("[TransportWebrtcSdk] str0m state changed {:?}", state);

        match state {
            IceConnectionState::New => {}
            IceConnectionState::Checking => {}
            IceConnectionState::Connected | IceConnectionState::Completed => {
                if let State::Reconnecting { at } = &self.state {
                    log::info!("[TransportWebrtcSdk] switched to reconnected after {:?}", now - *at);
                    self.state = State::Connected;
                    //TODO get paired ip
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected(self.remote)))));
                }
            }
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    log::info!("[TransportWebrtcSdk] switched to reconnecting");
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Reconnecting(
                            self.remote,
                        )))));
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
                    // If track don't have source, that mean it is empty sender, we need to wait attach request
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

    fn on_str0m_channel_event(&mut self, event: ClientEvent) {
        log::info!("[TransportWebrtcSdk] on client event {:?}", event);
        match return_if_none!(event.event) {
            protobuf::session::client_event::Event::Request(req) => match req.request {
                Some(protobuf::session::request::Request::Session(session)) => match session.request {
                    Some(session_req) => self.on_session_req(req.req_id, session_req),
                    None => self.send_rpc_res_err(req.req_id, RpcError::new2(WebrtcError::RpcInvalidRequest)),
                },
                Some(protobuf::session::request::Request::Sender(sender)) => match sender.request {
                    Some(sender_req) => self.on_sender_req(req.req_id, &sender.name, sender_req),
                    None => self.send_rpc_res_err(req.req_id, RpcError::new2(WebrtcError::RpcInvalidRequest)),
                },
                Some(protobuf::session::request::Request::Receiver(receiver)) => match receiver.request {
                    Some(receiver_req) => self.on_recever_req(req.req_id, &receiver.name, receiver_req),
                    None => self.send_rpc_res_err(req.req_id, RpcError::new2(WebrtcError::RpcInvalidRequest)),
                },
                Some(protobuf::session::request::Request::Room(room)) => match room.request {
                    Some(room_req) => self.on_room_req(req.req_id, room_req),
                    None => self.send_rpc_res_err(req.req_id, RpcError::new2(WebrtcError::RpcInvalidRequest)),
                },
                Some(protobuf::session::request::Request::Features(features_req)) => match features_req.request {
                    Some(protobuf::features::request::Request::Mixer(mixer_req)) => {
                        if let Some(mixer_req) = mixer_req.request {
                            self.on_mixer_req(req.req_id, mixer_req);
                        }
                    }
                    None => {}
                },
                None => self.send_rpc_res_err(req.req_id, RpcError::new2(WebrtcError::RpcInvalidRequest)),
            },
        }
    }
}

///This is for handling rpc from client
impl<ES: MediaEdgeSecure> TransportWebrtcSdk<ES> {
    fn on_session_req(&mut self, req_id: u32, req: protobuf::session::request::session::Request) {
        let build_req = |req: EndpointReq| InternalOutput::TransportOutput(TransportOutput::RpcReq(req_id.into(), req));
        match req {
            protobuf::session::request::session::Request::Join(req) => {
                let info = req.info.unwrap_or_default();
                let meta = PeerMeta { metadata: info.metadata };
                if let Some(token) = self.secure.decode_obj::<WebrtcToken>("webrtc", &req.token) {
                    if token.room == Some(info.room.clone()) && token.peer == Some(info.peer.clone()) {
                        let mixer_cfg = info.features.and_then(|f| {
                            f.mixer.and_then(|m| {
                                Some(AudioMixerConfig {
                                    mode: m.mode().into(),
                                    outputs: m.outputs.iter().map(|r| self.local_track_by_name(r.as_str()).map(|l| l.id())).flatten().collect::<Vec<_>>(),
                                    sources: m.sources.into_iter().map(|s| s.into()).collect::<Vec<_>>(),
                                })
                            })
                        });
                        self.queue.push_back(build_req(EndpointReq::JoinRoom(
                            info.room.into(),
                            info.peer.into(),
                            meta,
                            info.publish.unwrap_or_default().into(),
                            info.subscribe.unwrap_or_default().into(),
                            mixer_cfg,
                        )));
                    } else {
                        self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::RpcTokenRoomPeerNotMatch));
                    }
                } else {
                    self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::RpcTokenInvalid));
                }
            }
            protobuf::session::request::session::Request::Leave(_req) => self.queue.push_back(build_req(EndpointReq::LeaveRoom)),
            protobuf::session::request::session::Request::Sdp(req) => {
                let tracks = req.tracks.unwrap_or_default();
                for (index, s) in tracks.senders.into_iter().enumerate() {
                    if self.remote_track_by_name(&s.name).is_none() {
                        log::info!("[TransportWebrtcSdk] added new remote track {:?}", s);
                        self.remote_tracks.push(RemoteTrack::new((index as u16).into(), s));
                    }
                }

                for (index, r) in tracks.receivers.into_iter().enumerate() {
                    if self.local_track_by_name(&r.name).is_none() {
                        log::info!("[TransportWebrtcSdk] added new local track {:?}", r);
                        self.local_tracks.push(LocalTrack::new((index as u16).into(), r));
                    }
                }
                self.queue.push_back(InternalOutput::RpcReq(req_id, InternalRpcReq::SetRemoteSdp(req.sdp)));
            }
            protobuf::session::request::session::Request::Disconnect(_) => {
                log::info!("[TransportWebrtcSdk] switched to disconnected with close action from client");
                self.state = State::Disconnected;
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))))
            }
        }
    }

    fn on_sender_req(&mut self, req_id: u32, name: &str, req: protobuf::session::request::sender::Request) {
        let track = if let Some(track) = self.remote_track_by_name(name) {
            track
        } else {
            log::warn!("[TransportWebrtcSdk] request from unknown sender {}", name);
            return self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::RpcTrackNameNotFound));
        };
        let track_id = track.id();
        let build_req = |req: EndpointReq| InternalOutput::TransportOutput(TransportOutput::RpcReq(req_id.into(), req));

        match req {
            protobuf::session::request::sender::Request::Attach(attach) => {
                if !track.has_source() {
                    track.set_source(attach.source.unwrap_or_default());
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
                        protobuf::session::response::Response::Sender(protobuf::session::response::Sender {
                            response: Some(protobuf::session::response::sender::Response::Attach(protobuf::session::response::sender::Attach {})),
                        }),
                    );
                    self.queue.push_back(event);
                } else {
                    self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::RpcTrackAlreadyAttached));
                }
            }
            protobuf::session::request::sender::Request::Detach(_) => {
                if track.has_source() {
                    track.del_source();
                    let event = InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(track_id, RemoteTrackEvent::Ended)));
                    self.send_rpc_res(
                        req_id,
                        protobuf::session::response::Response::Sender(protobuf::session::response::Sender {
                            response: Some(protobuf::session::response::sender::Response::Detach(protobuf::session::response::sender::Detach {})),
                        }),
                    );
                    self.queue.push_back(event);
                } else {
                    self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::RpcTrackNotAttached));
                }
            }
            protobuf::session::request::sender::Request::Config(config) => self.queue.push_back(build_req(EndpointReq::RemoteTrack(track_id, EndpointRemoteTrackReq::Config(config.into())))),
        }
    }

    fn on_recever_req(&mut self, req_id: u32, name: &str, req: protobuf::session::request::receiver::Request) {
        let track = if let Some(track) = self.local_track_by_name(name) {
            track
        } else {
            log::warn!("[TransportWebrtcSdk] request from unknown receiver {}", name);
            return self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::RpcTrackNameNotFound));
        };
        let track_id = track.id();
        let build_req = |req: EndpointLocalTrackReq| InternalOutput::TransportOutput(TransportOutput::RpcReq(req_id.into(), EndpointReq::LocalTrack(track_id, req)));

        match req {
            protobuf::session::request::receiver::Request::Attach(attach) => {
                self.queue.push_back(build_req(EndpointLocalTrackReq::Attach(
                    attach.source.unwrap_or_default().into(),
                    attach.config.unwrap_or_default().into(),
                )));
            }
            protobuf::session::request::receiver::Request::Detach(_) => {
                self.queue.push_back(build_req(EndpointLocalTrackReq::Detach()));
            }
            protobuf::session::request::receiver::Request::Config(config) => {
                self.queue.push_back(build_req(EndpointLocalTrackReq::Config(config.into())));
            }
        }
    }

    fn on_mixer_req(&mut self, req_id: u32, req: protobuf::features::mixer::request::Request) {
        match req {
            protobuf::features::mixer::request::Request::Attach(req) => {
                let sources = req.sources.into_iter().map(|s| s.into()).collect::<Vec<_>>();
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    req_id.into(),
                    EndpointReq::AudioMixer(EndpointAudioMixerReq::Attach(sources)),
                )));
            }
            protobuf::features::mixer::request::Request::Detach(req) => {
                let sources = req.sources.into_iter().map(|s| s.into()).collect::<Vec<_>>();
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    req_id.into(),
                    EndpointReq::AudioMixer(EndpointAudioMixerReq::Detach(sources)),
                )));
            }
        }
    }

    fn on_room_req(&mut self, req_id: u32, req: protobuf::session::request::room::Request) {
        match req {
            protobuf::session::request::room::Request::ChannelControl(control) => {
                let label = control.label;
                let req = match control.control {
                    Some(RoomChannelControlReq::Control::Sub(_)) => Some(EndpointMessageChannelReq::Subscribe),
                    Some(RoomChannelControlReq::Control::Unsub(_)) => Some(EndpointMessageChannelReq::Unsubscribe),
                    Some(RoomChannelControlReq::Control::StartPub(_)) => Some(EndpointMessageChannelReq::StartPublish),
                    Some(RoomChannelControlReq::Control::StopPub(_)) => Some(EndpointMessageChannelReq::StopPublish),
                    Some(RoomChannelControlReq::Control::Pub(pub_data)) => Some(EndpointMessageChannelReq::PublishData(pub_data.data)),
                    None => None,
                };

                if let Some(req) = req {
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(req_id.into(), EndpointReq::MessageChannel(label, req))));
                }
            }
            _ => {
                // self.send_rpc_res_err(req_id, RpcError::new2(WebrtcError::RpcNotImplemented));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        sync::Arc,
        time::Instant,
    };

    use media_server_core::{
        endpoint::EndpointReq,
        transport::{TransportEvent, TransportOutput, TransportState},
    };
    use media_server_protocol::{
        endpoint::{PeerMeta, RoomInfoPublish, RoomInfoSubscribe},
        protobuf::{
            gateway,
            session::{self, client_event, ClientEvent},
            shared,
        },
        tokens::WebrtcToken,
    };
    use media_server_secure::{
        jwt::{MediaEdgeSecureJwt, MediaGatewaySecureJwt},
        MediaGatewaySecure,
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
            join: Some(session::RoomJoin {
                room: "room".to_string(),
                peer: "peer".to_string(),
                publish: Some(shared::RoomInfoPublish { peer: true, tracks: true }),
                subscribe: Some(shared::RoomInfoSubscribe { peers: true, tracks: true }),
                metadata: Some("metadata".to_string()),
                features: None,
            }),
            ..Default::default()
        };

        let channel_id = create_channel_id();

        let now = Instant::now();
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let secure_jwt = Arc::new(MediaEdgeSecureJwt::from(b"1234".as_slice()));
        let mut transport = TransportWebrtcSdk::new(req, secure_jwt.clone(), ip);
        assert_eq!(transport.pop_output(now), None);

        transport.on_tick(now);
        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connecting(ip)))))
        );

        transport.on_str0m_event(now, str0m::Event::ChannelOpen(channel_id, "data".to_string()));
        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected(ip)))))
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
                    RoomInfoSubscribe { peers: true, tracks: true },
                    None,
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
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let gateway_jwt = MediaGatewaySecureJwt::from(b"1234".as_slice());
        let secure_jwt = Arc::new(MediaEdgeSecureJwt::from(b"1234".as_slice()));
        let mut transport = TransportWebrtcSdk::new(req, secure_jwt.clone(), ip);
        assert_eq!(transport.pop_output(now), None);

        transport.on_tick(now);
        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connecting(ip)))))
        );

        transport.on_str0m_event(now, str0m::Event::ChannelOpen(channel_id, "data".to_string()));
        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected(ip)))))
        );
        assert_eq!(transport.pop_output(now), None);

        let token = gateway_jwt.encode_obj(
            "webrtc",
            WebrtcToken {
                room: Some("demo".to_string()),
                peer: Some("peer1".to_string()),
            },
            10000,
        );
        transport.on_str0m_channel_event(ClientEvent {
            seq: 0,
            event: Some(client_event::Event::Request(session::Request {
                req_id: 1,
                request: Some(session::request::Request::Session(session::request::Session {
                    request: Some(session::request::session::Request::Join(session::request::session::Join {
                        info: Some(session::RoomJoin {
                            room: "demo".to_string(),
                            peer: "peer1".to_string(),
                            metadata: None,
                            publish: None,
                            subscribe: None,
                            features: None,
                        }),
                        token: token.clone(),
                    })),
                })),
            })),
        });

        assert_eq!(
            transport.pop_output(now),
            Some(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                1.into(),
                EndpointReq::JoinRoom(
                    "demo".to_string().into(),
                    "peer1".to_string().into(),
                    PeerMeta { metadata: None },
                    RoomInfoPublish { peer: false, tracks: false },
                    RoomInfoSubscribe { peers: false, tracks: false },
                    None,
                )
            )))
        );
        assert_eq!(transport.pop_output(now), None);
    }

    //TODO test remote track non-source
    //TODO test remote track with source
    //TODO test remote track attach, detach
    //TODO test remote track lazy
    //TODO test local track
    //TODO test local track lazy
    //TODO test local track attach, detach
    //TODO test audio mixer event
}
