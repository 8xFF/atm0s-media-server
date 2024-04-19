use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use media_server_core::transport::{
    ClientEndpointEvent, ClientRemoteTrackEvent, RemoteTrackControl, RemoteTrackEvent, RemoteTrackId, TransportControl, TransportError, TransportEvent, TransportState,
};
use media_server_utils::Small2dMap;
use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};
use str0m::{
    media::{Direction, KeyframeRequestKind, MediaAdded, MediaKind, Mid},
    net::Protocol,
    Event as Str0mEvent, IceConnectionState, Output as Str0mOutput,
};

use crate::utils::rtp_to_media_packet;

use super::{ExtIn, InternalOutput, TransportWebrtcInternal};

const TIMEOUT_SEC: u64 = 10;
const AUDIO_TRACK: RemoteTrackId = RemoteTrackId(0);
const AUDIO_NAME: &str = "audio_main";
const VIDEO_TRACK: RemoteTrackId = RemoteTrackId(1);
const VIDEO_NAME: &str = "video_main";

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

pub struct TransportWebrtcWhip {
    next_tick: Option<Instant>,
    state: State,
    ports: Small2dMap<SocketAddr, usize>,
    audio_mid: Option<Mid>,
    video_mid: Option<Mid>,
}

impl TransportWebrtcWhip {
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

impl TransportWebrtcInternal for TransportWebrtcWhip {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        match &self.state {
            State::New => {
                self.state = State::Connecting { at: now };
                return Some(InternalOutput::TransportOutput(TransportEvent::State(TransportState::Connecting)));
            }
            State::Connecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("Connect timed out after {:?}", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    return Some(InternalOutput::TransportOutput(TransportEvent::State(TransportState::ConnectError(TransportError::Timeout))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("Reconnecting timed out after {:?}", now - *at);
                    self.state = State::Disconnected(Some(TransportWebrtcError::Timeout));
                    return Some(InternalOutput::TransportOutput(TransportEvent::State(TransportState::Disconnected(Some(TransportError::Timeout)))));
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

    fn on_transport_input<'a>(&mut self, now: Instant, input: TransportControl<'a, ExtIn>) -> Option<InternalOutput<'a>> {
        match input {
            TransportControl::Net(net) => match net {
                BackendIncoming::UdpPacket { slot, from, data } => {
                    let destination = self.ports.get2(&slot)?;
                    Some(InternalOutput::Str0mReceive(now, Protocol::Udp, from, *destination, data.freeze()))
                }
                _ => panic!("Unexpected input"),
            },
            TransportControl::Event(event) => self.on_endpoint_event(now, event),
            TransportControl::LocalMediaTrack(track, event) => None,
            TransportControl::RemoteMediaTrack(track, event) => match event {
                RemoteTrackControl::RequestKeyFrame => {
                    let mid = self.video_mid?;
                    Some(InternalOutput::Str0mKeyframe(mid, KeyframeRequestKind::Pli))
                }
            },
            TransportControl::Ext(_) => panic!("Unexpected ext input inside whip"),
            TransportControl::Close => panic!("Unexpected close input inside whip"),
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
                return Some(InternalOutput::TransportOutput(TransportEvent::Net(BackendOutgoing::UdpPacket {
                    slot: *from,
                    to: out.destination,
                    data: out.contents.to_vec().into(),
                })));
            }
            Str0mOutput::Event(event) => match event {
                Str0mEvent::Connected => {
                    self.state = State::Connected;
                    return Some(InternalOutput::TransportOutput(TransportEvent::State(TransportState::Connected)));
                }
                Str0mEvent::IceConnectionStateChange(state) => self.on_str0m_state(now, state),
                Str0mEvent::MediaAdded(media) => self.on_str0m_media_added(now, media),
                Str0mEvent::RtpPacket(pkt) => {
                    let track = if *pkt.header.payload_type == 111 {
                        AUDIO_TRACK
                    } else {
                        VIDEO_TRACK
                    };
                    let pkt = rtp_to_media_packet(pkt);
                    Some(InternalOutput::TransportOutput(TransportEvent::RemoteTrack(track, RemoteTrackEvent::Media(pkt))))
                }
                _ => None,
            },
        }
    }
}

impl TransportWebrtcWhip {
    fn on_str0m_state<'a>(&mut self, now: Instant, state: IceConnectionState) -> Option<InternalOutput<'a>> {
        match state {
            IceConnectionState::New => None,
            IceConnectionState::Checking => None,
            IceConnectionState::Connected | IceConnectionState::Completed => {
                if matches!(self.state, State::Reconnecting { at: _ }) {
                    self.state = State::Connected;
                    Some(InternalOutput::TransportOutput(TransportEvent::State(TransportState::Connected)))
                } else {
                    None
                }
            }
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    return Some(InternalOutput::TransportOutput(TransportEvent::State(TransportState::Reconnecting)));
                } else {
                    return None;
                }
            }
        }
    }

    fn on_str0m_media_added<'a>(&mut self, now: Instant, media: MediaAdded) -> Option<InternalOutput<'a>> {
        if matches!(media.direction, Direction::SendOnly | Direction::Inactive) {
            return None;
        }
        if media.kind == MediaKind::Audio {
            if self.audio_mid.is_some() {
                return None;
            }
            self.audio_mid = Some(media.mid);
            Some(InternalOutput::TransportOutput(TransportEvent::RemoteTrack(
                AUDIO_TRACK,
                RemoteTrackEvent::Started { name: AUDIO_NAME.to_string() },
            )))
        } else {
            if self.video_mid.is_some() {
                return None;
            }
            self.video_mid = Some(media.mid);
            Some(InternalOutput::TransportOutput(TransportEvent::RemoteTrack(
                VIDEO_TRACK,
                RemoteTrackEvent::Started { name: VIDEO_NAME.to_string() },
            )))
        }
    }

    fn on_endpoint_event<'a>(&mut self, now: Instant, event: ClientEndpointEvent) -> Option<InternalOutput<'a>> {
        match event {
            ClientEndpointEvent::PeerJoined(_) => None,
            ClientEndpointEvent::PeerLeaved(_) => None,
            ClientEndpointEvent::PeerTrackStarted(_, _, _) => None,
            ClientEndpointEvent::PeerTrackStopped(_, _) => None,
            ClientEndpointEvent::LocalTrack(_, _) => None,
            ClientEndpointEvent::RemoteTrack(_, ClientRemoteTrackEvent::LimitBitrateBps(bitrate)) => Some(InternalOutput::Str0mLimitBitrate(self.video_mid?, bitrate)),
            ClientEndpointEvent::RemoteTrack(_, ClientRemoteTrackEvent::RequestKeyFrame) => Some(InternalOutput::Str0mKeyframe(self.video_mid?, KeyframeRequestKind::Pli)),
        }
    }
}
