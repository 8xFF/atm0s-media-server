use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointLocalTrackConfig, EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointLocalTrackSource, EndpointReq},
    transport::{LocalTrackEvent, LocalTrackId, TransportError, TransportEvent, TransportOutput, TransportState},
};
use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackName, TrackPriority},
    media::MediaKind,
};
use sans_io_runtime::{collections::DynamicDeque, return_if_none};
use str0m::{
    bwe::BweKind,
    media::{Direction, MediaAdded, Mid},
    Event as Str0mEvent, IceConnectionState,
};

use super::{bwe_state::BweState, InternalOutput, TransportWebrtcInternal};

const TIMEOUT_SEC: u64 = 10;
const AUDIO_TRACK: LocalTrackId = LocalTrackId(0);
const VIDEO_TRACK: LocalTrackId = LocalTrackId(1);
const DEFAULT_PRIORITY: TrackPriority = TrackPriority(1);

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

#[derive(Default, Debug)]
struct SubscribeStreams {
    peer: Option<PeerId>,
    audio: Option<TrackName>,
    video: Option<TrackName>,
}

pub struct TransportWebrtcWhep {
    room: RoomId,
    peer: PeerId,
    state: State,
    audio_mid: Option<Mid>,
    video_mid: Option<Mid>,
    subscribed: SubscribeStreams,
    audio_subscribe_waits: VecDeque<(PeerId, TrackName, TrackMeta)>,
    video_subscribe_waits: VecDeque<(PeerId, TrackName, TrackMeta)>,
    bwe_state: BweState,
    queue: DynamicDeque<InternalOutput, 2>,
}

impl TransportWebrtcWhep {
    pub fn new(room: RoomId, peer: PeerId) -> Self {
        Self {
            room,
            peer,
            state: State::New,
            audio_mid: None,
            video_mid: None,
            subscribed: Default::default(),
            queue: Default::default(),
            audio_subscribe_waits: VecDeque::new(),
            video_subscribe_waits: VecDeque::new(),
            bwe_state: Default::default(),
        }
    }
}

impl TransportWebrtcInternal for TransportWebrtcWhep {
    fn on_codec_config(&mut self, _cfg: &str0m::format::CodecConfig) {}

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
                    log::info!("[TransportWebrtcWhep] connect timed out after {:?} => switched to ConnectError", now - *at);
                    self.state = State::ConnectError(TransportWebrtcError::Timeout);
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::ConnectError(
                            TransportError::Timeout,
                        )))));
                }
            }
            State::Reconnecting { at } => {
                if now - *at >= Duration::from_secs(TIMEOUT_SEC) {
                    log::info!("[TransportWebrtcWhep] reconnect timed out after {:?} => switched to Disconnected", now - *at);
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

    fn on_endpoint_event(&mut self, now: Instant, event: EndpointEvent) {
        match event {
            EndpointEvent::PeerJoined(_, _) => {}
            EndpointEvent::PeerLeaved(_, _) => {}
            EndpointEvent::PeerTrackStarted(peer, track, meta) => {
                if self.audio_mid.is_none() && meta.kind.is_audio() {
                    log::info!("[TransportWebrtcWhep] waiting local audio track => push Subscribe candidate to waits");
                    self.audio_subscribe_waits.push_back((peer, track, meta));
                    return;
                }
                if self.video_mid.is_none() && meta.kind.is_video() {
                    log::info!("[TransportWebrtcWhep] waiting local video track => push Subscribe candidate to waits");
                    self.video_subscribe_waits.push_back((peer, track, meta));
                    return;
                }
                self.try_subscribe(peer, track, meta);
            }
            EndpointEvent::PeerTrackStopped(peer, track, _meta) => self.try_unsubscribe(peer, track),
            EndpointEvent::LocalMediaTrack(_track, event) => match event {
                EndpointLocalTrackEvent::Media(pkt) => {
                    let mid = if pkt.meta.is_audio() {
                        return_if_none!(self.audio_mid)
                    } else {
                        let mid = return_if_none!(self.video_mid);
                        self.bwe_state.on_send_video(now);
                        mid
                    };
                    self.queue.push_back(InternalOutput::Str0mSendMedia(mid, pkt));
                }
            },
            EndpointEvent::RemoteMediaTrack(_track, _event) => {}
            EndpointEvent::BweConfig { current, desired } => {
                let (current, desired) = self.bwe_state.filter_bwe_config(current, desired);
                self.queue.push_back(InternalOutput::Str0mBwe(current, desired));
            }
            EndpointEvent::GoAway(_seconds, _reason) => {}
        }
    }

    fn on_str0m_event(&mut self, now: Instant, event: str0m::Event) {
        match event {
            Str0mEvent::Connected => {
                log::info!("[TransportWebrtcWhep] connected");
                self.state = State::Connected;
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(),
                    EndpointReq::JoinRoom(
                        self.room.clone(),
                        self.peer.clone(),
                        PeerMeta { metadata: None },
                        RoomInfoPublish { peer: false, tracks: false },
                        RoomInfoSubscribe { peers: false, tracks: true },
                    ),
                )));
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))));
            }
            Str0mEvent::IceConnectionStateChange(state) => self.on_str0m_state(now, state),
            Str0mEvent::MediaAdded(media) => self.on_str0m_media_added(now, media),
            Str0mEvent::KeyframeRequest(req) => {
                if self.video_mid == Some(req.mid) {
                    log::info!("[TransportWebrtcWhep] request key-frame");
                    self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                        VIDEO_TRACK,
                        LocalTrackEvent::RequestKeyFrame,
                    ))));
                }
            }
            Str0mEvent::EgressBitrateEstimate(BweKind::Remb(_, bitrate)) | Str0mEvent::EgressBitrateEstimate(BweKind::Twcc(bitrate)) => {
                let bitrate2 = self.bwe_state.filter_bwe(bitrate.as_u64());
                log::debug!("[TransportWebrtcWhep] on rewrite bwe {bitrate} => {bitrate2} bps");
                self.queue
                    .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::EgressBitrateEstimate(bitrate2))));
            }
            Str0mEvent::PeerStats(_stats) => {}
            Str0mEvent::MediaIngressStats(stats) => {
                log::debug!("[TransportWebrtcWhep] ingress rtt {} {:?}", stats.mid, stats.rtt);
            }
            Str0mEvent::MediaEgressStats(stats) => {
                log::debug!("[TransportWebrtcWhep] egress rtt {} {:?}", stats.mid, stats.rtt);
            }
            _ => {}
        }
    }

    fn close(&mut self, _now: Instant) {
        log::info!("[TransportWebrtcWhep] switched to disconnected with close action");
        self.state = State::Disconnected;
        self.queue
            .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None)))));
    }

    fn pop_output(&mut self, _now: Instant) -> Option<InternalOutput> {
        self.queue.pop_front()
    }
}

impl TransportWebrtcWhep {
    fn on_str0m_state(&mut self, now: Instant, state: IceConnectionState) {
        log::info!("[TransportWebrtcWhep] str0m state changed {:?}", state);

        match state {
            IceConnectionState::New => {}
            IceConnectionState::Checking => {}
            IceConnectionState::Connected | IceConnectionState::Completed => match &self.state {
                State::Reconnecting { at } => {
                    log::info!("[TransportWebrtcWhep] switched to reconnected after {:?}", now - *at);
                    self.state = State::Connected;
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Connected))))
                }
                _ => {}
            },
            IceConnectionState::Disconnected => {
                if matches!(self.state, State::Connected) {
                    self.state = State::Reconnecting { at: now };
                    log::info!("[TransportWebrtcWhep] switched to reconnecting");
                    self.queue
                        .push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::State(TransportState::Reconnecting))));
                }
            }
        }
    }

    fn on_str0m_media_added(&mut self, _now: Instant, media: MediaAdded) {
        log::info!("[TransportWebrtcWhep] str0m media added {:?}", media);
        if matches!(media.direction, Direction::RecvOnly | Direction::Inactive) {
            return;
        }
        if media.kind == str0m::media::MediaKind::Audio {
            if self.audio_mid.is_some() {
                return;
            }
            self.audio_mid = Some(media.mid);
            self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                AUDIO_TRACK,
                LocalTrackEvent::Started(MediaKind::Audio),
            ))));
            while let Some((peer, track, meta)) = self.audio_subscribe_waits.pop_front() {
                self.try_subscribe(peer, track, meta);
            }
        } else {
            if self.video_mid.is_some() {
                return;
            }
            self.video_mid = Some(media.mid);
            self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::LocalTrack(
                VIDEO_TRACK,
                LocalTrackEvent::Started(MediaKind::Video),
            ))));
            while let Some((peer, track, meta)) = self.video_subscribe_waits.pop_front() {
                self.try_subscribe(peer, track, meta);
            }
        }
    }
}

impl TransportWebrtcWhep {
    fn try_subscribe(&mut self, peer: PeerId, track: TrackName, meta: TrackMeta) {
        log::info!("[TransportWebrtcWhep] try subscribe {peer} {track}");
        if self.subscribed.peer.is_none() || self.subscribed.peer.eq(&Some(peer.clone())) {
            if self.subscribed.audio.is_none() && meta.kind.is_audio() {
                self.subscribed.peer = Some(peer.clone());
                self.subscribed.audio = Some(track.clone());
                log::info!("[TransportWebrtcWhep] send subscribe {peer} {track}");
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(), //TODO generate req_id
                    EndpointReq::LocalTrack(
                        AUDIO_TRACK,
                        EndpointLocalTrackReq::Attach(
                            EndpointLocalTrackSource { peer, track },
                            EndpointLocalTrackConfig {
                                priority: DEFAULT_PRIORITY,
                                max_spatial: 2,
                                max_temporal: 2,
                                min_spatial: None,
                                min_temporal: None,
                            },
                        ),
                    ),
                )));
                return;
            }

            if self.subscribed.video.is_none() && meta.kind.is_video() {
                self.subscribed.peer = Some(peer.clone());
                self.subscribed.video = Some(track.clone());
                log::info!("[TransportWebrtcWhep] send subscribe {peer} {track}");
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(), //TODO generate req_id
                    EndpointReq::LocalTrack(
                        VIDEO_TRACK,
                        EndpointLocalTrackReq::Attach(
                            EndpointLocalTrackSource { peer, track },
                            EndpointLocalTrackConfig {
                                priority: DEFAULT_PRIORITY,
                                max_spatial: 2,
                                max_temporal: 2,
                                min_spatial: None,
                                min_temporal: None,
                            },
                        ),
                    ),
                )));
                return;
            }
        }
    }

    //TODO try to get other tracks if available
    fn try_unsubscribe(&mut self, peer: PeerId, track: TrackName) {
        log::info!("[TransportWebrtcWhep] try unsubscribe {peer} {track}, current subscribed {:?}", self.subscribed);
        if self.subscribed.peer.eq(&Some(peer.clone())) {
            if self.subscribed.audio.eq(&Some(track.clone())) {
                self.subscribed.audio = None;
                log::info!("[TransportWebrtcWhep] send unsubscribe {peer} {track}");
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(), //TODO generate req_id
                    EndpointReq::LocalTrack(AUDIO_TRACK, EndpointLocalTrackReq::Detach()),
                )));
            }

            if self.subscribed.video.eq(&Some(track.clone())) {
                self.subscribed.video = None;
                log::info!("[TransportWebrtcWhep] send unsubscribe {peer} {track}");
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(), //TODO generate req_id
                    EndpointReq::LocalTrack(VIDEO_TRACK, EndpointLocalTrackReq::Detach()),
                )));
            }

            if self.subscribed.audio.is_none() && self.subscribed.video.is_none() {
                self.subscribed.peer = None;
            }
        }
    }
}
