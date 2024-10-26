use std::{
    collections::VecDeque,
    net::IpAddr,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointReq},
    transport::{RemoteTrackEvent, RemoteTrackId, TransportError, TransportEvent, TransportOutput, TransportState},
};
use media_server_protocol::{
    endpoint::{BitrateControlMode, PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackPriority},
    media::{MediaKind, MediaScaling},
};
use sans_io_runtime::return_if_none;
use str0m::{
    format::CodecConfig,
    media::{Direction, KeyframeRequestKind, MediaAdded, Mid},
    Event as Str0mEvent, IceConnectionState,
};

use crate::media::RemoteMediaConvert;

use super::{InternalOutput, TransportWebrtcInternal};

const TIMEOUT_SEC: u64 = 10;
const AUDIO_TRACK: RemoteTrackId = RemoteTrackId::build(0);
const AUDIO_NAME: &str = "audio_main";
const VIDEO_TRACK: RemoteTrackId = RemoteTrackId::build(1);
const VIDEO_NAME: &str = "video_main";

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

pub struct TransportWebrtcWhip {
    remote: IpAddr,
    room: RoomId,
    peer: PeerId,
    extra_data: Option<String>,
    state: State,
    audio_mid: Option<Mid>,
    ///mid and simulcast flag
    video_mid: Option<(Mid, bool)>,
    queue: VecDeque<InternalOutput>,
    media_convert: RemoteMediaConvert,
}

impl TransportWebrtcWhip {
    pub fn new(room: RoomId, peer: PeerId, extra_data: Option<String>, remote: IpAddr) -> Self {
        Self {
            remote,
            room,
            peer,
            extra_data,
            state: State::New,
            audio_mid: None,
            video_mid: None,
            queue: VecDeque::new(),
            media_convert: RemoteMediaConvert::default(),
        }
    }
}

impl TransportWebrtcInternal for TransportWebrtcWhip {
    fn on_codec_config(&mut self, cfg: &CodecConfig) {
        self.media_convert.set_config(cfg);
    }

    fn on_tick(&mut self, now: Instant) {
        match &self.state {
            State::New => {
                self.state = State::Connecting { at: now };
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connecting(self.remote)))));
            }
            State::Connecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcWhip] connect timed out after {:?} => switched to ConnectError", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::ConnectError(
                            TransportError::Timeout,
                        )))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcWhip] reconnect timed out after {:?} => switched to Disconnected", now - *at);
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

    fn on_rpc_res(&mut self, _req_id: u32, _res: media_server_protocol::transport::RpcResult<super::InternalRpcRes>) {}

    fn on_transport_rpc_res(&mut self, _now: Instant, _req_id: media_server_core::endpoint::EndpointReqId, _res: media_server_core::endpoint::EndpointRes) {}

    fn on_endpoint_event(&mut self, _now: Instant, event: EndpointEvent) {
        match event {
            EndpointEvent::PeerJoined(_, _) => {}
            EndpointEvent::PeerLeaved(_, _) => {}
            EndpointEvent::PeerTrackStarted(_, _, _) => {}
            EndpointEvent::PeerTrackStopped(_, _, _) => {}
            EndpointEvent::RemoteMediaTrack(_, event) => match event {
                media_server_core::endpoint::EndpointRemoteTrackEvent::RequestKeyFrame => {
                    let mid = return_if_none!(self.video_mid).0;
                    log::info!("[TransportWebrtcWhip] request key-frame");
                    self.queue.push_back(InternalOutput::Str0mKeyframe(mid, KeyframeRequestKind::Fir));
                }
                media_server_core::endpoint::EndpointRemoteTrackEvent::LimitBitrateBps { min, max } => {
                    let (mid, sim) = return_if_none!(self.video_mid);
                    let bitrate = if sim {
                        max
                    } else {
                        min
                    };
                    log::debug!("[TransportWebrtcWhip] limit video track {mid} with bitrate {bitrate} bps");
                    self.queue.push_back(InternalOutput::Str0mLimitBitrate(mid, bitrate));
                }
            },
            EndpointEvent::LocalMediaTrack(_, _) => {}
            EndpointEvent::BweConfig { .. } => {}
            EndpointEvent::GoAway(_, _) => {}
            EndpointEvent::AudioMixer(_) => {}
            EndpointEvent::ChannelMessage(..) => {}
        }
    }

    fn on_str0m_event(&mut self, now: Instant, event: Str0mEvent) {
        match event {
            Str0mEvent::Connected => {
                self.state = State::Connected;
                log::info!("[TransportWebrtcWhip] connected");
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected(self.remote)))));
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(),
                    EndpointReq::JoinRoom(
                        self.room.clone(),
                        self.peer.clone(),
                        PeerMeta {
                            metadata: None,
                            extra_data: self.extra_data.clone(),
                        },
                        RoomInfoPublish { peer: true, tracks: true },
                        RoomInfoSubscribe { peers: false, tracks: false },
                        None,
                    ),
                )));
            }
            Str0mEvent::IceConnectionStateChange(state) => self.on_str0m_state(now, state),
            Str0mEvent::MediaAdded(media) => self.on_str0m_media_added(now, media),
            Str0mEvent::RtpPacket(pkt) => {
                let track = if *pkt.header.payload_type == 111 {
                    AUDIO_TRACK
                } else {
                    VIDEO_TRACK
                };
                let pkt = return_if_none!(self.media_convert.convert(pkt));
                log::trace!(
                    "[TransportWebrtcWhip] incoming pkt codec {:?}, seq {} ts {}, marker {}, payload {}",
                    pkt.meta,
                    pkt.seq,
                    pkt.ts,
                    pkt.marker,
                    pkt.data.len(),
                );
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                    track,
                    RemoteTrackEvent::Media(pkt),
                ))))
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
        log::info!("[TransportWebrtcWhip] switched to disconnected with close action");
        self.state = State::Disconnected;
        self.queue
            .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))));
    }

    fn pop_output(&mut self, _now: Instant) -> Option<InternalOutput> {
        self.queue.pop_front()
    }
}

impl TransportWebrtcWhip {
    fn on_str0m_state(&mut self, now: Instant, state: IceConnectionState) {
        log::info!("[TransportWebrtcWhip] str0m state changed {:?}", state);

        match state {
            IceConnectionState::New => {}
            IceConnectionState::Checking => {}
            IceConnectionState::Connected | IceConnectionState::Completed => {
                if let State::Reconnecting { at } = &self.state {
                    log::info!("[TransportWebrtcWhip] switched to reconnected after {:?}", now - *at);
                    self.state = State::Connected;
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected(self.remote)))));
                }
            }
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    log::info!("[TransportWebrtcWhip] switched to reconnecting");
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Reconnecting(
                            self.remote,
                        )))));
                }
            }
        }
    }

    fn on_str0m_media_added(&mut self, _now: Instant, media: MediaAdded) {
        log::info!("[TransportWebrtcWhip] str0m media added {:?}", media);
        if matches!(media.direction, Direction::SendOnly | Direction::Inactive) {
            return;
        }
        if media.kind == str0m::media::MediaKind::Audio {
            if self.audio_mid.is_some() {
                return;
            }
            self.audio_mid = Some(media.mid);
            log::info!("[TransportWebrtcWhip] started remote track {AUDIO_NAME}");
            self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                AUDIO_TRACK,
                RemoteTrackEvent::Started {
                    name: AUDIO_NAME.to_string(),
                    meta: TrackMeta {
                        kind: MediaKind::Audio,
                        scaling: MediaScaling::None,
                        control: BitrateControlMode::MaxBitrate,
                        metadata: None,
                    },
                    priority: TrackPriority::from(1),
                },
            ))));
        } else {
            if self.video_mid.is_some() {
                return;
            }
            self.video_mid = Some((media.mid, media.simulcast.is_some()));
            log::info!("[TransportWebrtcWhip] started remote track {VIDEO_NAME}");
            self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                VIDEO_TRACK,
                RemoteTrackEvent::Started {
                    name: VIDEO_NAME.to_string(),
                    meta: TrackMeta {
                        kind: MediaKind::Video,
                        scaling: if media.simulcast.is_some() {
                            MediaScaling::Simulcast
                        } else {
                            MediaScaling::None
                        },
                        control: BitrateControlMode::MaxBitrate,
                        metadata: None,
                    },
                    priority: TrackPriority::from(1),
                },
            ))));
        }
    }
}

#[cfg(test)]
mod tests {
    //TODO test handle str0m connected event
    //TODO test handle str0m state changed event
    //TODO test handle str0m track started event
    //TODO test handle endpoint event: request key-frame
    //TODO test handle endpoint event: limit bitrate
    //TODO test handle close request
}
