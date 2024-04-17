use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use media_server_core::transport::{LocalTrackControl, LocalTrackEvent, RemoteTrackControl, RemoteTrackEvent, TransportError, TransportInput, TransportOutput, TransportState};
use media_server_utils::Small2dMap;
use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};
use str0m::{
    media::{Direction, KeyframeRequestKind, MediaKind, Mid},
    net::Protocol,
    Event as Str0mEvent, IceConnectionState, Output as Str0mOutput,
};

use super::{InternalOutput, TransportWebrtcInternal};

const TIMEOUT_SEC: u64 = 10;
const AUDIO_TRACK: u16 = 0;
const AUDIO_NAME: &str = "audio0";
const VIDEO_TRACK: u16 = 1;
const VIDEO_NAME: &str = "video0";

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

pub struct TransportWebrtcWhep {
    next_tick: Option<Instant>,
    state: State,
    ports: Small2dMap<SocketAddr, usize>,
    audio_mid: Option<Mid>,
    video_mid: Option<Mid>,
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
        }
    }
}

impl TransportWebrtcInternal for TransportWebrtcWhep {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        match &self.state {
            State::New => {
                self.state = State::Connecting { at: now };
                return Some(InternalOutput::TransportOutput(TransportOutput::State(TransportState::Connecting)));
            }
            State::Connecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("Connect timed out after {:?}", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    return Some(InternalOutput::TransportOutput(TransportOutput::State(TransportState::ConnectError(TransportError::Timeout))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("Reconnecting timed out after {:?}", now - *at);
                    self.state = State::Disconnected(Some(TransportWebrtcError::Timeout));
                    return Some(InternalOutput::TransportOutput(TransportOutput::State(TransportState::Disconnected(Some(TransportError::Timeout)))));
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

    fn on_transport_input<'a>(&mut self, now: Instant, input: TransportInput<'a>) -> Option<InternalOutput<'a>> {
        match input {
            TransportInput::Net(net) => match net {
                BackendIncoming::UdpPacket { slot, from, data } => {
                    let destination = self.ports.get2(&slot)?;
                    Some(InternalOutput::Str0mReceive(now, Protocol::Udp, from, *destination, data.freeze()))
                }
                _ => panic!("Unexpected input"),
            },
            TransportInput::Control(control) => match control {},
            TransportInput::LocalMediaTrack(track, control) => match control {
                LocalTrackControl::Media(pkt) => {
                    let mid = match track {
                        AUDIO_TRACK => self.audio_mid,
                        VIDEO_TRACK => self.video_mid,
                        _ => None,
                    }?;
                    Some(InternalOutput::Str0mSendMedia(mid, pkt))
                }
            },
            TransportInput::RemoteMediaTrack(track, event) => match event {
                RemoteTrackControl::RequestKeyFrame => {
                    let mid = self.video_mid?;
                    Some(InternalOutput::Str0mKeyframe(mid, KeyframeRequestKind::Pli))
                }
            },
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
                    return Some(InternalOutput::TransportOutput(TransportOutput::State(TransportState::Connected)));
                }
                Str0mEvent::IceConnectionStateChange(state) => match state {
                    IceConnectionState::New => None,
                    IceConnectionState::Checking => None,
                    IceConnectionState::Connected | IceConnectionState::Completed => {
                        if matches!(self.state, State::Reconnecting { at: _ }) {
                            self.state = State::Connected;
                            Some(InternalOutput::TransportOutput(TransportOutput::State(TransportState::Connected)))
                        } else {
                            None
                        }
                    }
                    IceConnectionState::Disconnected => {
                        if matches!(self.state, State::Connected) {
                            self.state = State::Reconnecting { at: now };
                            return Some(InternalOutput::TransportOutput(TransportOutput::State(TransportState::Reconnecting)));
                        } else {
                            return None;
                        }
                    }
                },
                Str0mEvent::MediaAdded(media) => {
                    if matches!(media.direction, Direction::RecvOnly | Direction::Inactive) {
                        return None;
                    }
                    if media.kind == MediaKind::Audio {
                        if self.audio_mid.is_some() {
                            return None;
                        }
                        self.audio_mid = Some(media.mid);
                        Some(InternalOutput::TransportOutput(TransportOutput::LocalTrack(
                            AUDIO_TRACK,
                            LocalTrackEvent::Started { name: AUDIO_NAME.to_string() },
                        )))
                    } else {
                        if self.video_mid.is_some() {
                            return None;
                        }
                        self.video_mid = Some(media.mid);
                        Some(InternalOutput::TransportOutput(TransportOutput::RemoteTrack(
                            VIDEO_TRACK,
                            RemoteTrackEvent::Started { name: VIDEO_NAME.to_string() },
                        )))
                    }
                }
                Str0mEvent::KeyframeRequest(req) => {
                    if self.video_mid == Some(req.mid) {
                        Some(InternalOutput::TransportOutput(TransportOutput::LocalTrack(VIDEO_TRACK, LocalTrackEvent::RequestKeyFrame)))
                    } else {
                        None
                    }
                }
                _ => None,
            },
        }
    }
}
