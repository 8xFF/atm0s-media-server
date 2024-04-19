use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointLocalTrackEvent},
    transport::{LocalTrackEvent, LocalTrackId, TransportError, TransportEvent, TransportInput, TransportOutput, TransportState},
};
use media_server_protocol::endpoint::{PeerId, TrackMeta, TrackName};
use media_server_utils::Small2dMap;
use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};
use str0m::{
    media::{Direction, MediaAdded, MediaKind, Mid},
    net::Protocol,
    Event as Str0mEvent, IceConnectionState, Output as Str0mOutput,
};

use super::{ExtIn, InternalOutput, TransportWebrtcInternal};

const TIMEOUT_SEC: u64 = 10;
const AUDIO_TRACK: LocalTrackId = LocalTrackId(0);
const VIDEO_TRACK: LocalTrackId = LocalTrackId(1);

enum State {
    New,
    Connecting { at: Instant },
    ConnectError(TransportWebrtcError),
    Connected,
    Reconnecting { at: Instant },
    Disconnected(Option<TransportWebrtcError>),
}

enum TransportWebrtcError {
    Timeout,
}

#[derive(Default)]
struct SubscribeStreams {
    peer: Option<PeerId>,
    audio: Option<TrackName>,
    video: Option<TrackName>,
}

pub struct TransportWebrtcWhep {
    next_tick: Option<Instant>,
    state: State,
    ports: Small2dMap<SocketAddr, usize>,
    audio_mid: Option<Mid>,
    video_mid: Option<Mid>,
    subscribed: SubscribeStreams,
}

impl TransportWebrtcWhep {
    pub fn new(local_addrs: Vec<(SocketAddr, usize)>) -> Self {
        let mut ports = Small2dMap::default();
        for (local_addr, slot) in local_addrs {
            ports.insert(local_addr, slot);
        }
        Self {
            state: State::New,
            next_tick: None,
            ports,
            audio_mid: None,
            video_mid: None,
            subscribed: Default::default(),
        }
    }
}

impl TransportWebrtcInternal for TransportWebrtcWhep {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        match &self.state {
            State::New => {
                self.state = State::Connecting { at: now };
                return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connecting))));
            }
            State::Connecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("Connect timed out after {:?}", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::ConnectError(
                        TransportError::Timeout,
                    )))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("Reconnecting timed out after {:?}", now - *at);
                    self.state = State::Disconnected(Some(TransportWebrtcError::Timeout));
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(Some(
                        TransportError::Timeout,
                    ))))));
                }
            }
            _ => {}
        }
        let next_tick = self.next_tick?;
        if next_tick > now {
            return None;
        }
        self.next_tick = None;
        Some(InternalOutput::Str0mTick(now))
    }

    fn on_transport_input<'a>(&mut self, now: Instant, input: TransportInput<'a, ExtIn>) -> Option<InternalOutput<'a>> {
        match input {
            TransportInput::Net(net) => match net {
                BackendIncoming::UdpPacket { slot, from, data } => {
                    let destination = self.ports.get2(&slot)?;
                    Some(InternalOutput::Str0mReceive(now, Protocol::Udp, from, *destination, data.freeze()))
                }
                _ => panic!("Unexpected input"),
            },
            TransportInput::Endpoint(event) => self.on_endpoint_event(now, event),
            TransportInput::Ext(_) => panic!("Unexpected ext input inside whep"),
            TransportInput::Close => panic!("Unexpected close input inside whep"),
            TransportInput::RpcRes(_, _) => todo!(),
        }
    }

    fn on_str0m_out<'a>(&mut self, now: Instant, out: Str0mOutput) -> Option<InternalOutput<'a>> {
        match out {
            Str0mOutput::Timeout(instance) => {
                self.next_tick = Some(instance);
                None
            }
            Str0mOutput::Transmit(out) => {
                let from = self.ports.get1(&out.source)?;
                return Some(InternalOutput::TransportOutput(TransportOutput::Net(BackendOutgoing::UdpPacket {
                    slot: *from,
                    to: out.destination,
                    data: out.contents.to_vec().into(),
                })));
            }
            Str0mOutput::Event(event) => match event {
                Str0mEvent::Connected => {
                    self.state = State::Connected;
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))));
                }
                Str0mEvent::IceConnectionStateChange(state) => self.on_str0m_state(now, state),
                Str0mEvent::MediaAdded(media) => self.on_str0m_media_added(now, media),
                Str0mEvent::KeyframeRequest(req) => {
                    if self.video_mid == Some(req.mid) {
                        Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                            VIDEO_TRACK,
                            LocalTrackEvent::RequestKeyFrame,
                        ))))
                    } else {
                        None
                    }
                }
                _ => None,
            },
        }
    }
}

impl TransportWebrtcWhep {
    fn on_str0m_state<'a>(&mut self, now: Instant, state: IceConnectionState) -> Option<InternalOutput<'a>> {
        match state {
            IceConnectionState::New => None,
            IceConnectionState::Checking => None,
            IceConnectionState::Connected | IceConnectionState::Completed => {
                if matches!(self.state, State::Reconnecting { at: _ }) {
                    self.state = State::Connected;
                    Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
                } else {
                    None
                }
            }
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Reconnecting))));
                } else {
                    return None;
                }
            }
        }
    }

    fn on_str0m_media_added<'a>(&mut self, now: Instant, media: MediaAdded) -> Option<InternalOutput<'a>> {
        if matches!(media.direction, Direction::RecvOnly | Direction::Inactive) {
            return None;
        }
        if media.kind == MediaKind::Audio {
            if self.audio_mid.is_some() {
                return None;
            }
            self.audio_mid = Some(media.mid);
            None
        } else {
            if self.video_mid.is_some() {
                return None;
            }
            self.video_mid = Some(media.mid);
            None
        }
    }

    fn on_endpoint_event<'a>(&mut self, now: Instant, event: EndpointEvent) -> Option<InternalOutput<'a>> {
        match event {
            EndpointEvent::PeerJoined(_) => None,
            EndpointEvent::PeerLeaved(_) => None,
            EndpointEvent::PeerTrackStarted(peer, track, meta) => self.try_subscribe(peer, track, meta),
            EndpointEvent::PeerTrackStopped(peer, track) => self.try_unsubscribe(peer, track),
            EndpointEvent::LocalMediaTrack(track, event) => match event {
                EndpointLocalTrackEvent::Media(pkt) => Some(InternalOutput::Str0mSendMedia(self.video_mid?, pkt)),
            },
            EndpointEvent::RemoteMediaTrack(track, event) => None,
        }
    }
}

impl TransportWebrtcWhep {
    fn try_subscribe<'a>(&mut self, peer: PeerId, track: TrackName, meta: TrackMeta) -> Option<InternalOutput<'a>> {
        if self.subscribed.peer.is_none() || self.subscribed.peer.eq(&Some(peer.clone())) {
            if self.subscribed.audio.is_none() && meta.kind.is_audio() {
                self.subscribed.audio = Some(track.clone());
                return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                    AUDIO_TRACK,
                    LocalTrackEvent::Switch(Some((peer, track))),
                ))));
            }

            if self.subscribed.video.is_none() && meta.kind.is_audio() {
                self.subscribed.video = Some(track.clone());
                return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                    VIDEO_TRACK,
                    LocalTrackEvent::Switch(Some((peer, track))),
                ))));
            }
        }

        None
    }

    fn try_unsubscribe<'a>(&mut self, peer: PeerId, track: TrackName) -> Option<InternalOutput<'a>> {
        if self.subscribed.peer.eq(&Some(peer.clone())) {
            if self.subscribed.audio.eq(&Some(track.clone())) {
                self.subscribed.audio = None;
                return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                    AUDIO_TRACK,
                    LocalTrackEvent::Switch(None),
                ))));
            }

            if self.subscribed.video.eq(&Some(track)) {
                self.subscribed.video = None;
                return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                    VIDEO_TRACK,
                    LocalTrackEvent::Switch(None),
                ))));
            }

            if self.subscribed.audio.is_none() && self.subscribed.video.is_none() {
                self.subscribed.peer = None;
            }
        }

        None
    }
}
