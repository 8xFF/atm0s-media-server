use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::EndpointEvent,
    transport::{RemoteTrackEvent, RemoteTrackId, TransportError, TransportEvent, TransportOutput, TransportState},
};
use str0m::{
    media::{Direction, KeyframeRequestKind, MediaAdded, MediaKind, Mid},
    Event as Str0mEvent, IceConnectionState,
};

use crate::utils::rtp_to_media_packet;

use super::{InternalOutput, TransportWebrtcInternal};

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
    state: State,
    audio_mid: Option<Mid>,
    video_mid: Option<Mid>,
    queue: VecDeque<InternalOutput<'static>>,
}

impl TransportWebrtcWhip {
    pub fn new() -> Self {
        Self {
            state: State::New,
            audio_mid: None,
            video_mid: None,
            queue: VecDeque::new(),
        }
    }
}

impl TransportWebrtcInternal for TransportWebrtcWhip {
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
        None
    }

    fn on_endpoint_event<'a>(&mut self, now: Instant, event: EndpointEvent) -> Option<InternalOutput<'a>> {
        match event {
            EndpointEvent::PeerJoined(_) => todo!(),
            EndpointEvent::PeerLeaved(_) => todo!(),
            EndpointEvent::PeerTrackStarted(_, _, _) => todo!(),
            EndpointEvent::PeerTrackStopped(_, _) => todo!(),
            EndpointEvent::RemoteMediaTrack(_, event) => match event {
                media_server_core::endpoint::EndpointRemoteTrackEvent::RequestKeyFrame => {
                    let mid = self.video_mid?;
                    Some(InternalOutput::Str0mKeyframe(mid, KeyframeRequestKind::Pli))
                }
                media_server_core::endpoint::EndpointRemoteTrackEvent::LimitBitrateBps(bitrate) => {
                    let mid = self.video_mid?;
                    Some(InternalOutput::Str0mLimitBitrate(mid, bitrate))
                }
            },
            EndpointEvent::LocalMediaTrack(_, _) => todo!(),
        }
    }

    fn on_transport_rpc_res<'a>(&mut self, now: Instant, req_id: media_server_core::endpoint::EndpointReqId, res: media_server_core::endpoint::EndpointRes) -> Option<InternalOutput<'a>> {
        None
    }

    fn on_str0m_event<'a>(&mut self, now: Instant, event: Str0mEvent) -> Option<InternalOutput<'a>> {
        match event {
            Str0mEvent::Connected => {
                self.state = State::Connected;
                return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))));
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
                Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                    track,
                    RemoteTrackEvent::Media(pkt),
                ))))
            }
            _ => None,
        }
    }

    fn close<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        self.queue.push_back(InternalOutput::Destroy);
        log::info!("[TransportWebrtcWhep] switched to disconnected with close action");
        self.state = State::Disconnected(None);
        Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))))
    }

    fn pop_output<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
        self.queue.pop_front()
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
        if matches!(media.direction, Direction::SendOnly | Direction::Inactive) {
            return None;
        }
        if media.kind == MediaKind::Audio {
            if self.audio_mid.is_some() {
                return None;
            }
            self.audio_mid = Some(media.mid);
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                AUDIO_TRACK,
                RemoteTrackEvent::Started { name: AUDIO_NAME.to_string() },
            ))))
        } else {
            if self.video_mid.is_some() {
                return None;
            }
            self.video_mid = Some(media.mid);
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                VIDEO_TRACK,
                RemoteTrackEvent::Started { name: VIDEO_NAME.to_string() },
            ))))
        }
    }
}
