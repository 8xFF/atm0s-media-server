use std::collections::VecDeque;

use cluster::rpc::webrtc::{WebrtcConnectRequestSender, WebrtcRemoteIceResponse};
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use media_utils::{HashMapMultiKey, RtpSeqExtend, StringCompression};
use str0m::{
    media::{Direction, MediaKind, Simulcast},
    rtp::SeqNo,
    IceConnectionState,
};
use transport::{
    LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, RemoteTrackIncomingEvent, RemoteTrackOutgoingEvent, RequestKeyframeKind, TrackId, TrackMeta, TransportError, TransportIncomingEvent,
    TransportOutgoingEvent, TransportRuntimeError,
};

use self::{
    local_track_id_generator::LocalTrackIdGenerator,
    rpc::{rpc_from_string, rpc_local_track_to_string, rpc_remote_track_to_string, rpc_to_string, IncomingRpc, TransportRpcIn, TransportRpcOut},
    track_info_queue::TrackInfoQueue,
    utils::to_transport_kind,
};
use crate::{transport::internal::rpc::rpc_internal_to_string, TransportLifeCycle};

use super::WebrtcTransportEvent;

mod local_track_id_generator;
pub(crate) mod rpc;
mod track_info_queue;
pub(crate) mod utils;

#[derive(Default)]
struct LocalTrack {
    active: bool,
    req_extender: RtpSeqExtend,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Str0mInput {
    Connected,
    ChannelOpen(usize, String),
    ChannelData(usize, bool, Vec<u8>),
    ChannelClosed(usize),
    IceConnectionStateChange(IceConnectionState),
    MediaPacket(TrackId, MediaPacket),
    MediaAdded(Direction, TrackId, MediaKind, Option<Simulcast>),
    MediaChanged(Direction, TrackId),
    KeyframeRequest(TrackId, RequestKeyframeKind),
    EgressBitrateEstimate(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Str0mAction {
    Media(TrackId, SeqNo, MediaPacket),
    RequestKeyFrame(TrackId, RequestKeyframeKind),
    Datachannel(usize, String),
    Rpc(TransportRpcIn),
    ConfigEgressBitrate { current: u32, desired: u32 },
    LimitIngressBitrate { track_id: TrackId, max: u32 },
    RemoteIce(String),
    Close,
}

pub struct WebrtcTransportInternal<L>
where
    L: TransportLifeCycle,
{
    life_cycle: L,
    track_info_queue: TrackInfoQueue,
    local_track_id_map: HashMapMultiKey<TrackId, String, LocalTrack>,
    remote_track_id_map: HashMapMultiKey<TrackId, String, bool>,
    remote_audio_track_ids: Vec<TrackId>,
    remote_video_track_ids: Vec<TrackId>,
    local_track_id_gen: LocalTrackIdGenerator,
    channel_id: Option<usize>,
    channel_pending_msgs: VecDeque<String>,
    endpoint_actions: VecDeque<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>>,
    str0m_actions: VecDeque<Str0mAction>,
    string_compression: StringCompression,
}

impl<L> WebrtcTransportInternal<L>
where
    L: TransportLifeCycle,
{
    pub fn new(life_cycle: L) -> Self {
        log::info!("[TransportWebrtcInternal] created");

        Self {
            life_cycle,
            track_info_queue: Default::default(),
            local_track_id_map: Default::default(),
            remote_track_id_map: Default::default(),
            local_track_id_gen: Default::default(),
            remote_audio_track_ids: Default::default(),
            remote_video_track_ids: Default::default(),
            channel_id: None,
            channel_pending_msgs: Default::default(),
            endpoint_actions: Default::default(),
            str0m_actions: Default::default(),
            string_compression: StringCompression::default(),
        }
    }

    pub fn map_remote_stream(&mut self, sender: WebrtcConnectRequestSender) {
        self.track_info_queue.add(&sender.uuid, &sender.label, sender.kind, &sender.name);
    }

    fn send_msg(&mut self, msg: String) {
        if let Some(channel_id) = self.channel_id {
            self.str0m_actions.push_back(Str0mAction::Datachannel(channel_id, msg));
        } else {
            self.channel_pending_msgs.push_back(msg);
        }
    }

    fn restore_msgs(&mut self) {
        assert!(self.channel_id.is_some());
        while let Some(msg) = self.channel_pending_msgs.pop_front() {
            if let Some(channel_id) = self.channel_id {
                self.str0m_actions.push_back(Str0mAction::Datachannel(channel_id, msg));
            }
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.life_cycle.on_tick(now_ms);
        self.pop_life_cycle();
        Ok(())
    }

    pub fn on_endpoint_event(&mut self, now_ms: u64, event: TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>) -> Result<(), TransportError> {
        match event {
            TransportOutgoingEvent::LocalTrackEvent(track_id, event) => match event {
                LocalTrackOutgoingEvent::MediaPacket(pkt) => {
                    if let Some((slot, _)) = self.local_track_id_map.get_mut_by_k1(&track_id) {
                        if let Some(ext_seq) = slot.req_extender.generate(pkt.seq_no) {
                            self.str0m_actions.push_back(Str0mAction::Media(track_id, ext_seq.into(), pkt));
                        }
                    }
                }
                LocalTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_local_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on local track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RemoteTrackEvent(track_id, event) => match event {
                RemoteTrackOutgoingEvent::RequestKeyFrame(kind) => {
                    log::info!("[TransportWebrtc] request keyframe with video track_id {}", track_id);
                    self.str0m_actions.push_back(Str0mAction::RequestKeyFrame(track_id, kind));
                }
                RemoteTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_remote_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on remote track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::LimitIngressBitrate(bitrate) => {
                if let Some(track_id) = self.remote_video_track_ids.first() {
                    log::debug!("[TransportWebrtc] request ingress bitrate: {} with first video track_id {}", bitrate, track_id);
                    self.str0m_actions.push_back(Str0mAction::LimitIngressBitrate {
                        track_id: track_id.clone(),
                        max: bitrate,
                    });
                }
            }
            TransportOutgoingEvent::ConfigEgressBitrate { current, desired } => {
                log::debug!("[TransportWebrtc] config egress bitrate: {} {}", current, desired);
                self.str0m_actions.push_back(Str0mAction::ConfigEgressBitrate { current, desired });
            }
            TransportOutgoingEvent::Rpc(rpc) => {
                let msg = rpc_to_string(rpc);
                log::info!("[TransportWebrtc] on endpoint out rpc: {}", msg);
                self.send_msg(msg);
            }
        }
        Ok(())
    }

    pub fn on_custom_event(&mut self, _now_ms: u64, event: WebrtcTransportEvent) -> Result<(), TransportError> {
        match event {
            WebrtcTransportEvent::RemoteIce(req) => {
                self.str0m_actions.push_back(Str0mAction::RemoteIce(req.param().candidate.clone()));
                req.answer(Ok(WebrtcRemoteIceResponse { success: true }));
            }
            WebrtcTransportEvent::SdpPatch(req) => req.answer(Err("NOT_IMPLEMENTED")),
        }
        Ok(())
    }

    pub fn on_transport_rpc(&mut self, _now_ms: u64, rpc: TransportRpcOut) {
        let msg = rpc_internal_to_string(rpc);
        log::info!("[TransportWebrtc] on transport out rpc: {}", msg);
        self.send_msg(msg);
    }

    pub fn on_str0m_event(&mut self, now_ms: u64, event: Str0mInput) -> Result<(), TransportError> {
        self.life_cycle.on_transport_event(now_ms, &event);
        self.pop_life_cycle();

        match event {
            Str0mInput::Connected => Ok(()),
            Str0mInput::ChannelOpen(channel_id, _name) => {
                self.channel_id = Some(channel_id);
                self.restore_msgs();
                Ok(())
            }
            Str0mInput::ChannelData(_channel_id, binary, data) => {
                let msg = match binary {
                    true => self.string_compression.uncompress_zlib(&data),
                    false => String::from_utf8(data).ok(),
                };
                if let Some(data) = msg {
                    match rpc_from_string(&data) {
                        Ok(IncomingRpc::Endpoint(rpc)) => {
                            log::info!("[TransportWebrtcInternal] on incoming endpoint rpc: [{:?}]", rpc);
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::Rpc(rpc)));
                            Ok(())
                        }
                        Ok(IncomingRpc::Transport(rpc)) => {
                            log::info!("[TransportWebrtcInternal] on incoming transport sdp rpc: [{:?}]", rpc);
                            self.str0m_actions.push_back(Str0mAction::Rpc(rpc));
                            Ok(())
                        }
                        Ok(IncomingRpc::LocalTrack(track_name, rpc)) => {
                            if let Some((_, track_id)) = self.local_track_id_map.get_by_k2(&track_name) {
                                log::info!("[TransportWebrtcInternal] on incoming local track[{}] rpc: [{:?}]", track_name, rpc);
                                self.endpoint_actions
                                    .push_back(Ok(TransportIncomingEvent::LocalTrackEvent(*track_id, LocalTrackIncomingEvent::Rpc(rpc))));
                                Ok(())
                            } else {
                                log::warn!("[TransportWebrtcInternal] on incoming local invalid track[{}] rpc: [{:?}]", track_name, rpc);
                                Err(TransportError::RuntimeError(TransportRuntimeError::TrackIdNotFound))
                            }
                        }
                        Ok(IncomingRpc::RemoteTrack(track_name, rpc)) => {
                            if let Some((_, track_id)) = self.remote_track_id_map.get_by_k2(&track_name) {
                                log::info!("[TransportWebrtcInternal] on incoming remote track[{}] rpc: [{:?}]", track_name, rpc);
                                self.endpoint_actions
                                    .push_back(Ok(TransportIncomingEvent::RemoteTrackEvent(*track_id, RemoteTrackIncomingEvent::Rpc(rpc))));
                                Ok(())
                            } else {
                                log::warn!("[TransportWebrtcInternal] on incoming remote invalid track[{}] rpc: [{:?}]", track_name, rpc);
                                Err(TransportError::RuntimeError(TransportRuntimeError::TrackIdNotFound))
                            }
                        }
                        _ => {
                            log::warn!("[TransportWebrtcInternal] invalid rpc: {}", data);
                            Err(TransportError::RuntimeError(TransportRuntimeError::RpcInvalid))
                        }
                    }
                } else {
                    Err(TransportError::RuntimeError(TransportRuntimeError::RpcInvalid))
                }
            }
            Str0mInput::ChannelClosed(_channel_id) => Ok(()),
            Str0mInput::IceConnectionStateChange(_state) => Ok(()),
            Str0mInput::MediaPacket(track_id, pkt) => {
                self.endpoint_actions
                    .push_back(Ok(TransportIncomingEvent::RemoteTrackEvent(track_id, RemoteTrackIncomingEvent::MediaPacket(pkt))));
                Ok(())
            }
            Str0mInput::MediaAdded(direction, track_id, kind, _sim) => {
                match direction {
                    Direction::RecvOnly | Direction::SendRecv => {
                        match kind {
                            MediaKind::Audio => {
                                self.remote_audio_track_ids.push(track_id);
                            }
                            MediaKind::Video => {
                                self.remote_video_track_ids.push(track_id);
                            }
                        }
                        //remote stream
                        if let Some(info) = self.track_info_queue.pop(kind) {
                            self.remote_track_id_map.insert(track_id, info.name.clone(), true);
                            log::info!("[TransportWebrtcInternal] added remote track {} => {} added {:?}", info.name, track_id, info);
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::RemoteTrackAdded(
                                info.name,
                                track_id,
                                TrackMeta::from_kind(to_transport_kind(kind), Some(info.label)),
                            )));
                            Ok(())
                        } else {
                            log::warn!("[TransportWebrtcInternal] added remote track {} track_id {} but missing info", kind, track_id);
                            Err(TransportError::RuntimeError(TransportRuntimeError::TrackIdNotFound))
                        }
                    }
                    Direction::SendOnly => {
                        //local stream
                        let track_name = self.local_track_id_gen.generate(kind, track_id);
                        log::info!("[TransportWebrtcInternal] added local track {} => {} ", track_name, track_id);
                        self.local_track_id_map.insert(track_id, track_name.clone(), LocalTrack { active: true, ..Default::default() });
                        self.endpoint_actions
                            .push_back(Ok(TransportIncomingEvent::LocalTrackAdded(track_name, track_id, TrackMeta::from_kind(to_transport_kind(kind), None))));
                        Ok(())
                    }
                    _ => {
                        log::error!("[TransportWebrtcInternal] not support direction {:?} for track {}", direction, track_id);
                        Ok(())
                    }
                }
            }
            Str0mInput::MediaChanged(direction, track_id) => {
                match direction {
                    Direction::Inactive => {
                        if let Some((active, name)) = self.remote_track_id_map.get_mut_by_k1(&track_id) {
                            log::info!("[TransportWebrtcInternal] switched remote to inactive {}", track_id);
                            *active = false;
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::RemoteTrackRemoved(name.clone(), track_id)));
                        } else if let Some((slot, name)) = self.local_track_id_map.get_mut_by_k1(&track_id) {
                            log::info!("[TransportWebrtcInternal] switched local to inactive {}", track_id);
                            slot.active = false;
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::LocalTrackRemoved(name.clone(), track_id)));
                        } else {
                            log::warn!("[TransportWebrtcInternal] switch track to inactive {} but cannot determine remote or local", track_id);
                        }
                    }
                    Direction::RecvOnly => {
                        //TODO
                    }
                    Direction::SendOnly => {
                        //TODO
                    }
                    Direction::SendRecv => {
                        //Not support
                    }
                }
                Ok(())
            }
            Str0mInput::KeyframeRequest(track_id, kind) => {
                self.endpoint_actions
                    .push_back(Ok(TransportIncomingEvent::LocalTrackEvent(track_id, LocalTrackIncomingEvent::RequestKeyFrame(kind))));
                Ok(())
            }
            Str0mInput::EgressBitrateEstimate(bitrate) => {
                log::debug!("[TransportWebrtcInternal] on egress bitrate estimate {} bps", bitrate);
                self.endpoint_actions.push_back(Ok(TransportIncomingEvent::EgressBitrateEstimate(bitrate)));
                Ok(())
            }
        }
    }

    pub fn close(&mut self) {
        self.str0m_actions.push_back(Str0mAction::Close);
        self.endpoint_actions.push_back(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Disconnected)));
    }

    pub fn endpoint_action(&mut self) -> Option<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>> {
        self.endpoint_actions.pop_front()
    }

    pub fn str0m_action(&mut self) -> Option<Str0mAction> {
        self.str0m_actions.pop_front()
    }

    fn pop_life_cycle(&mut self) {
        while let Some(out) = self.life_cycle.pop_action() {
            self.endpoint_actions.push_back(out.map(|state| TransportIncomingEvent::State(state)));
        }
    }
}

impl<L: TransportLifeCycle> Drop for WebrtcTransportInternal<L> {
    fn drop(&mut self) {
        log::info!("[TransportWebrtcInternal] drop");
    }
}

#[cfg(test)]
mod test {
    use cluster::rpc::webrtc::WebrtcConnectRequestSender;
    use endpoint::{rpc::TrackInfo, EndpointRpcOut};
    use str0m::media::{Direction, MediaKind};
    use transport::{MediaPacket, TransportIncomingEvent};

    use crate::{transport::internal::Str0mInput, TransportWithDatachannelLifeCycle};

    use super::WebrtcTransportInternal;

    fn create_connected_internal() -> WebrtcTransportInternal<TransportWithDatachannelLifeCycle> {
        let mut internal = WebrtcTransportInternal::new(TransportWithDatachannelLifeCycle::new(0));

        // we need wait both webrtc and datachannel connected
        internal.on_str0m_event(100, Str0mInput::Connected).unwrap();
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);
        internal.on_str0m_event(100, Str0mInput::ChannelOpen(0, "data".to_string())).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Connected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal
    }

    #[test]
    fn simple_flow_webrtc_connected() {
        let mut internal = create_connected_internal();

        internal.on_str0m_event(100, Str0mInput::ChannelClosed(0)).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Disconnected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.map_remote_stream(WebrtcConnectRequestSender {
            kind: transport::MediaKind::Audio,
            name: "audio_main".to_string(),
            uuid: "track_id".to_string(),
            label: "label".to_string(),
            screen: None,
        });
        internal.on_str0m_event(100, Str0mInput::MediaAdded(Direction::RecvOnly, 100, MediaKind::Audio, None)).unwrap();

        assert_eq!(
            internal.endpoint_action(),
            Some(Ok(TransportIncomingEvent::RemoteTrackAdded(
                "audio_main".to_string(),
                100,
                transport::TrackMeta::from_kind(transport::MediaKind::Audio, Some("label".to_string())),
            )))
        );

        internal.on_str0m_event(100, Str0mInput::MediaChanged(Direction::Inactive, 100)).unwrap();

        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::RemoteTrackRemoved("audio_main".to_string(), 100,))));
    }

    #[test]
    fn simple_flow_webrtc_connection_reconnect() {
        let mut internal = create_connected_internal();

        internal.on_str0m_event(100, Str0mInput::IceConnectionStateChange(str0m::IceConnectionState::Disconnected)).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Reconnecting))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.on_str0m_event(100, Str0mInput::IceConnectionStateChange(str0m::IceConnectionState::Connected)).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Reconnected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);
    }

    #[test]
    fn simple_flow_webrtc_connection_failed() {
        let mut internal = create_connected_internal();

        internal.on_str0m_event(100, Str0mInput::IceConnectionStateChange(str0m::IceConnectionState::Disconnected)).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Reconnecting))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.on_tick(100 + 29999).unwrap();
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.on_tick(100 + 30000).unwrap();
        assert_eq!(
            internal.endpoint_action(),
            Some(Err(transport::TransportError::ConnectionError(transport::ConnectionErrorReason::Timeout)))
        );
    }

    #[test]
    fn simple_flow_webrtc_connect_timeout() {
        let mut internal = WebrtcTransportInternal::new(TransportWithDatachannelLifeCycle::new(0));

        internal.on_tick(9999).unwrap();
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.on_tick(10000).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Err(transport::TransportError::ConnectError(transport::ConnectErrorReason::Timeout))));
    }

    #[test]
    fn invalid_rpc() {
        let mut internal = create_connected_internal();

        assert_eq!(
            internal.on_str0m_event(1000, Str0mInput::ChannelData(0, false, "".as_bytes().to_vec())),
            Err(transport::TransportError::RuntimeError(transport::TransportRuntimeError::RpcInvalid))
        );
    }

    #[test]
    fn should_fire_rpc() {
        let mut internal = create_connected_internal();

        let req_str = r#"{"req_id":1,"type":"request","request":"peer.close"}"#;
        assert_eq!(internal.on_str0m_event(1000, Str0mInput::ChannelData(0, false, req_str.as_bytes().to_vec())), Ok(()));
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::Rpc(endpoint::rpc::EndpointRpcIn::PeerClose))));
        assert_eq!(internal.endpoint_action(), None);
    }

    #[test]
    fn outgoing_rpc_must_wait_connected() {
        let mut internal = WebrtcTransportInternal::new(TransportWithDatachannelLifeCycle::new(0));

        internal
            .on_endpoint_event(10, transport::TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo::new_audio("test", "track", None))))
            .unwrap();
        assert_eq!(internal.str0m_action(), None);

        // we need wait both webrtc and datachannel connected
        internal.on_str0m_event(100, Str0mInput::Connected).unwrap();
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.on_str0m_event(100, Str0mInput::ChannelOpen(0, "data".to_string())).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Connected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(
            internal.str0m_action(),
            Some(crate::transport::Str0mAction::Datachannel(
                0,
                "{\"data\":{\"kind\":\"audio\",\"peer\":\"test\",\"peer_hash\":854508108,\"state\":null,\"stream\":\"track\"},\"event\":\"stream_added\",\"type\":\"event\"}".to_string()
            ))
        );
    }

    #[test]
    fn simple_flow_webrtc_remote_pkt() {
        let mut internal = create_connected_internal();

        internal.map_remote_stream(WebrtcConnectRequestSender {
            kind: transport::MediaKind::Audio,
            name: "audio_main".to_string(),
            uuid: "track_id".to_string(),
            label: "label".to_string(),
            screen: None,
        });

        internal.on_str0m_event(1000, Str0mInput::MediaAdded(Direction::RecvOnly, 100, MediaKind::Audio, None)).unwrap();

        // must fire remote track added to endpoint
        assert_eq!(
            internal.endpoint_action(),
            Some(Ok(TransportIncomingEvent::RemoteTrackAdded(
                "audio_main".to_string(),
                100,
                transport::TrackMeta::from_kind(transport::MediaKind::Audio, Some("label".to_string())),
            )))
        );
        assert_eq!(internal.endpoint_action(), None);

        // must fire remote track pkt to endpoint
        {
            let pkt = MediaPacket::simple_audio(1, 1000, vec![1, 2, 3]);
            internal.on_str0m_event(2000, Str0mInput::MediaPacket(100, pkt.clone())).unwrap();

            assert_eq!(
                internal.endpoint_action(),
                Some(Ok(TransportIncomingEvent::RemoteTrackEvent(100, transport::RemoteTrackIncomingEvent::MediaPacket(pkt))))
            );
        }
    }
}
