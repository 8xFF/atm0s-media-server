use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointReq},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportError, TransportEvent, TransportOutput, TransportState},
};
use media_server_protocol::{
    endpoint::{BitrateControlMode, PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackPriority},
    media::{MediaKind, MediaScaling},
    protobuf::gateway::ConnectRequest,
};
use str0m::{
    format::CodecConfig,
    media::{Direction, KeyframeRequestKind, MediaAdded, Mid},
    Event as Str0mEvent, IceConnectionState,
};

use crate::media::RemoteMediaConvert;

use self::{local_track::LocalTrack, remote_track::RemoteTrack};

use super::{InternalOutput, TransportWebrtcInternal};

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
    join: Option<(RoomId, PeerId, RoomInfoPublish, RoomInfoSubscribe)>,
    state: State,
    queue: VecDeque<InternalOutput<'static>>,
    local_tracks: Vec<LocalTrack>,
    remote_tracks: Vec<RemoteTrack>,
    media_convert: RemoteMediaConvert,
}

impl TransportWebrtcSdk {
    pub fn new(req: ConnectRequest) -> Self {
        Self {
            join: req.join.map(|j| (j.room.into(), j.peer.into(), j.publish.into(), j.subscribe.into())),
            state: State::New,
            local_tracks: req.tracks.receivers.into_iter().enumerate().map(|(index, r)| LocalTrack::new((index as u16).into(), r)).collect(),
            remote_tracks: req.tracks.senders.into_iter().enumerate().map(|(index, s)| RemoteTrack::new((index as u16).into(), s)).collect(),
            queue: VecDeque::new(),
            media_convert: RemoteMediaConvert::default(),
        }
    }

    fn remote_track(&mut self, track_id: RemoteTrackId) -> Option<&mut RemoteTrack> {
        self.remote_tracks.iter_mut().find(|t| t.id() == track_id)
    }

    fn remote_track_by_mid(&mut self, mid: Mid) -> Option<&mut RemoteTrack> {
        self.remote_tracks.iter_mut().find(|t| t.mid() == Some(mid))
    }

    fn local_track(&mut self, track_id: LocalTrackId) -> Option<&mut LocalTrack> {
        self.local_tracks.iter_mut().find(|t| t.id() == track_id)
    }
}

impl TransportWebrtcInternal for TransportWebrtcSdk {
    fn on_codec_config(&mut self, cfg: &CodecConfig) {}

    fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>> {
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

    fn on_endpoint_event<'a>(&mut self, _now: Instant, event: EndpointEvent) -> Option<InternalOutput<'a>> {
        match event {
            EndpointEvent::PeerJoined(_, _) => None,
            EndpointEvent::PeerLeaved(_) => None,
            EndpointEvent::PeerTrackStarted(_, _, _) => None,
            EndpointEvent::PeerTrackStopped(_, _) => None,
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
            Str0mEvent::ChannelOpen(_channel, name) => {
                self.state = State::Connected;
                log::info!("[TransportWebrtcSdk] channel opened, join state {:?}", self.join);
                if let Some((room, peer, publish, subscribe)) = &self.join {
                    self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                        0.into(),
                        EndpointReq::JoinRoom(room.clone(), peer.clone(), PeerMeta {}, publish.clone(), subscribe.clone()),
                    )));
                }
                Some(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
            }
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
                        LocalTrackEvent::Started(MediaKind::Audio),
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
}

#[cfg(test)]
mod tests {}
