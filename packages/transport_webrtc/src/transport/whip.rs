use std::{
    collections::VecDeque,
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
use str0m::{
    media::{Direction, KeyframeRequestKind, MediaAdded, Mid},
    Event as Str0mEvent, IceConnectionState,
};

use crate::utils::rtp_to_media_packet;

use super::{InternalOutput, TransportWebrtcInternal};

const TIMEOUT_SEC: u64 = 10;
const AUDIO_TRACK: RemoteTrackId = RemoteTrackId(0);
const AUDIO_NAME: &str = "audio_main";
const VIDEO_TRACK: RemoteTrackId = RemoteTrackId(1);
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
    room: RoomId,
    peer: PeerId,
    state: State,
    audio_mid: Option<Mid>,
    video_mid: Option<Mid>,
    queue: VecDeque<InternalOutput<'static>>,
}

impl TransportWebrtcWhip {
    pub fn new(room: RoomId, peer: PeerId) -> Self {
        Self {
            room,
            peer,
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
                    log::info!("[TransportWebrtcWhip] connect timed out after {:?} => switched to ConnectError", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::ConnectError(
                        TransportError::Timeout,
                    )))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcWhip] reconnect timed out after {:?} => switched to Disconnected", now - *at);
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

    fn on_endpoint_event<'a>(&mut self, _now: Instant, event: EndpointEvent) -> Option<InternalOutput<'a>> {
        match event {
            EndpointEvent::PeerJoined(_, _) => None,
            EndpointEvent::PeerLeaved(_) => None,
            EndpointEvent::PeerTrackStarted(_, _, _) => None,
            EndpointEvent::PeerTrackStopped(_, _) => None,
            EndpointEvent::RemoteMediaTrack(_, event) => match event {
                media_server_core::endpoint::EndpointRemoteTrackEvent::RequestKeyFrame => {
                    let mid = self.video_mid?;
                    log::info!("[TransportWebrtcWhip] request key-frame");
                    Some(InternalOutput::Str0mKeyframe(mid, KeyframeRequestKind::Pli))
                }
                media_server_core::endpoint::EndpointRemoteTrackEvent::LimitBitrateBps(bitrate) => {
                    let mid = self.video_mid?;
                    Some(InternalOutput::Str0mLimitBitrate(mid, bitrate))
                }
            },
            EndpointEvent::LocalMediaTrack(_, _) => None,
            EndpointEvent::BweConfig { .. } => None,
            EndpointEvent::GoAway(_, _) => None,
        }
    }

    fn on_transport_rpc_res<'a>(&mut self, _now: Instant, _req_id: media_server_core::endpoint::EndpointReqId, _res: media_server_core::endpoint::EndpointRes) -> Option<InternalOutput<'a>> {
        None
    }

    fn on_str0m_event<'a>(&mut self, now: Instant, event: Str0mEvent) -> Option<InternalOutput<'a>> {
        match event {
            Str0mEvent::Connected => {
                self.state = State::Connected;
                log::info!("[TransportWebrtcWhip] connected");
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(),
                    EndpointReq::JoinRoom(
                        self.room.clone(),
                        self.peer.clone(),
                        PeerMeta {},
                        RoomInfoPublish { peer: true, tracks: true },
                        RoomInfoSubscribe { peers: false, tracks: false },
                    ),
                )));
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
        log::info!("[TransportWebrtcWhip] switched to disconnected with close action");
        self.state = State::Disconnected;
        Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))))
    }

    fn pop_output<'a>(&mut self, _now: Instant) -> Option<InternalOutput<'a>> {
        self.queue.pop_front()
    }
}

impl TransportWebrtcWhip {
    fn on_str0m_state<'a>(&mut self, now: Instant, state: IceConnectionState) -> Option<InternalOutput<'a>> {
        log::info!("[TransportWebrtcWhip] str0m state changed {:?}", state);

        match state {
            IceConnectionState::New => None,
            IceConnectionState::Checking => None,
            IceConnectionState::Connected | IceConnectionState::Completed => match &self.state {
                State::Reconnecting { at } => {
                    log::info!("[TransportWebrtcWhip] switched to reconnected after {:?}", now - *at);
                    self.state = State::Connected;
                    Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
                }
                _ => None,
            },
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    log::info!("[TransportWebrtcWhip] switched to reconnecting");
                    return Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Reconnecting))));
                } else {
                    return None;
                }
            }
        }
    }

    fn on_str0m_media_added<'a>(&mut self, _now: Instant, media: MediaAdded) -> Option<InternalOutput<'a>> {
        log::info!("[TransportWebrtcWhip] str0m media added {:?}", media);
        if matches!(media.direction, Direction::SendOnly | Direction::Inactive) {
            return None;
        }
        if media.kind == str0m::media::MediaKind::Audio {
            if self.audio_mid.is_some() {
                return None;
            }
            self.audio_mid = Some(media.mid);
            log::info!("[TransportWebrtcWhip] started remote track {AUDIO_NAME}");
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                AUDIO_TRACK,
                RemoteTrackEvent::Started {
                    name: AUDIO_NAME.to_string(),
                    meta: TrackMeta {
                        kind: MediaKind::Audio,
                        scaling: MediaScaling::None,
                        control: BitrateControlMode::NonControl,
                    },
                    priority: TrackPriority(1),
                },
            ))))
        } else {
            if self.video_mid.is_some() {
                return None;
            }
            self.video_mid = Some(media.mid);
            log::info!("[TransportWebrtcWhip] started remote track {VIDEO_NAME}");
            Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                VIDEO_TRACK,
                RemoteTrackEvent::Started {
                    name: VIDEO_NAME.to_string(),
                    meta: TrackMeta {
                        kind: MediaKind::Video,
                        scaling: MediaScaling::None,
                        control: BitrateControlMode::MaxBitrate,
                    },
                    priority: TrackPriority(1),
                },
            ))))
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
