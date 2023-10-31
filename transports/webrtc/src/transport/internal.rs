use std::collections::VecDeque;

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use media_utils::HashMapMultiKey;
use str0m::{
    media::{Direction, MediaKind, Mid, Simulcast},
    IceConnectionState,
};
use transport::{
    LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, RemoteTrackIncomingEvent, RemoteTrackOutgoingEvent, RequestKeyframeKind, TrackId, TrackMeta, TransportError, TransportIncomingEvent,
    TransportOutgoingEvent, TransportRuntimeError,
};

use self::{
    life_cycle::{life_cycle_event_to_event, TransportLifeCycle},
    local_track_id_generator::LocalTrackIdGenerator,
    rpc::{rpc_from_string, rpc_local_track_to_string, rpc_remote_track_to_string, rpc_to_string, IncomingRpc, TransportRpcIn, TransportRpcOut},
    string_compression::StringCompression,
    track_info_queue::TrackInfoQueue,
    utils::to_transport_kind,
};
use crate::{
    rpc::{WebrtcConnectRequestSender, WebrtcRemoteIceRequest},
    transport::internal::rpc::rpc_internal_to_string,
};

use super::{
    mid_convert::{mid_to_track, track_to_mid},
    WebrtcTransportEvent,
};

pub(crate) mod life_cycle;
mod local_track_id_generator;
pub(crate) mod rpc;
mod string_compression;
mod track_info_queue;
pub(crate) mod utils;

#[derive(Debug, PartialEq, Eq)]
pub enum Str0mInput {
    Connected,
    ChannelOpen(usize, String),
    ChannelData(usize, bool, Vec<u8>),
    ChannelClosed(usize),
    IceConnectionStateChange(IceConnectionState),
    MediaPacket(TrackId, MediaPacket),
    MediaAdded(Direction, Mid, MediaKind, Option<Simulcast>),
    MediaChanged(Direction, Mid),
    KeyframeRequest(Mid, RequestKeyframeKind),
    EgressBitrateEstimate(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Str0mAction {
    Media(Mid, MediaPacket),
    RequestKeyFrame(Mid, RequestKeyframeKind),
    Datachannel(usize, String),
    Rpc(TransportRpcIn),
    ConfigEgressBitrate { current: u32, desired: u32 },
    RemoteIce(WebrtcRemoteIceRequest, transport::RpcResponse<()>),
}

pub struct WebrtcTransportInternal<L>
where
    L: TransportLifeCycle,
{
    life_cycle: L,
    track_info_queue: TrackInfoQueue,
    local_track_id_map: HashMapMultiKey<TrackId, String, bool>,
    remote_track_id_map: HashMapMultiKey<TrackId, String, bool>,
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
            channel_id: None,
            channel_pending_msgs: Default::default(),
            endpoint_actions: Default::default(),
            str0m_actions: Default::default(),
            string_compression: StringCompression::default(),
        }
    }

    pub fn map_remote_stream(&mut self, sender: WebrtcConnectRequestSender) {
        self.track_info_queue.add(&sender.uuid, &sender.label, &sender.kind, &sender.name);
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
        if let Some(e) = self.life_cycle.on_tick(now_ms) {
            log::info!("[TransportWebrtc] on new state on tick {:?}", e);
            life_cycle_event_to_event(Some(e), &mut self.endpoint_actions);
        }
        Ok(())
    }

    pub fn on_endpoint_event(&mut self, _now_ms: u64, event: TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>) -> Result<(), TransportError> {
        match event {
            TransportOutgoingEvent::LocalTrackEvent(track_id, event) => match event {
                LocalTrackOutgoingEvent::MediaPacket(pkt) => {
                    let mid = track_to_mid(track_id);
                    self.str0m_actions.push_back(Str0mAction::Media(mid, pkt));
                }
                LocalTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_local_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on local track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RemoteTrackEvent(track_id, event) => match event {
                RemoteTrackOutgoingEvent::RequestKeyFrame(kind) => {
                    let mid = track_to_mid(track_id);
                    self.str0m_actions.push_back(Str0mAction::RequestKeyFrame(mid, kind));
                }
                RemoteTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_remote_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on remote track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RequestIngressBitrate(_bitrate) => {
                //TODO
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
            WebrtcTransportEvent::RemoteIce(ice, res) => {
                self.str0m_actions.push_back(Str0mAction::RemoteIce(ice, res));
            }
        }
        Ok(())
    }

    pub fn on_transport_rpc(&mut self, _now_ms: u64, rpc: TransportRpcOut) {
        let msg = rpc_internal_to_string(rpc);
        log::info!("[TransportWebrtc] on transport out rpc: {}", msg);
        self.send_msg(msg);
    }

    pub fn on_str0m_event(&mut self, now_ms: u64, event: Str0mInput) -> Result<(), TransportError> {
        match event {
            Str0mInput::Connected => {
                life_cycle_event_to_event(self.life_cycle.on_webrtc_connected(now_ms), &mut self.endpoint_actions);
                Ok(())
            }
            Str0mInput::ChannelOpen(chanel_id, _name) => {
                self.channel_id = Some(chanel_id);
                self.restore_msgs();
                life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, true), &mut self.endpoint_actions);
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
            Str0mInput::ChannelClosed(_chanel_id) => {
                life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, false), &mut self.endpoint_actions);
                Ok(())
            }
            Str0mInput::IceConnectionStateChange(state) => {
                life_cycle_event_to_event(self.life_cycle.on_ice_state(now_ms, state), &mut self.endpoint_actions);
                Ok(())
            }
            Str0mInput::MediaPacket(track_id, pkt) => {
                self.endpoint_actions
                    .push_back(Ok(TransportIncomingEvent::RemoteTrackEvent(track_id, RemoteTrackIncomingEvent::MediaPacket(pkt))));
                Ok(())
            }
            Str0mInput::MediaAdded(direction, mid, kind, _sim) => {
                match direction {
                    Direction::RecvOnly => {
                        //remote stream
                        let track_id = mid_to_track(&mid);
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
                            Err(TransportError::RuntimeError(TransportRuntimeError::TrackIdNotFound))
                        }
                    }
                    Direction::SendOnly => {
                        //local stream
                        let track_id = mid_to_track(&mid);
                        let track_name = self.local_track_id_gen.generate(kind, mid);
                        log::info!("[TransportWebrtcInternal] added local track {} => {} ", track_name, track_id);
                        self.local_track_id_map.insert(track_id, track_name.clone(), true);
                        self.endpoint_actions
                            .push_back(Ok(TransportIncomingEvent::LocalTrackAdded(track_name, track_id, TrackMeta::from_kind(to_transport_kind(kind), None))));
                        Ok(())
                    }
                    _ => {
                        panic!("not supported")
                    }
                }
            }
            Str0mInput::MediaChanged(direction, mid) => {
                match direction {
                    Direction::Inactive => {
                        let track_id = mid_to_track(&mid);
                        if let Some((active, name)) = self.remote_track_id_map.get_mut_by_k1(&track_id) {
                            log::info!("[TransportWebrtcInternal] switched remote to inactive {}", mid);
                            *active = false;
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::RemoteTrackRemoved(name.clone(), track_id)));
                        } else if let Some((active, name)) = self.local_track_id_map.get_mut_by_k1(&track_id) {
                            log::info!("[TransportWebrtcInternal] switched local to inactive {}", mid);
                            *active = false;
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::LocalTrackRemoved(name.clone(), track_id)));
                        } else {
                            log::warn!("[TransportWebrtcInternal] switch track to inactive {} but cannot determine remote or local", mid);
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
            Str0mInput::KeyframeRequest(mid, kind) => {
                let track_id = mid_to_track(&mid);
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

    pub fn endpoint_action(&mut self) -> Option<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>> {
        self.endpoint_actions.pop_front()
    }

    pub fn str0m_action(&mut self) -> Option<Str0mAction> {
        self.str0m_actions.pop_front()
    }
}

impl<L: TransportLifeCycle> Drop for WebrtcTransportInternal<L> {
    fn drop(&mut self) {
        log::info!("[TransportWebrtcInternal] drop");
    }
}

#[cfg(test)]
mod test {
    use endpoint::{rpc::TrackInfo, EndpointRpcOut};
    use str0m::media::{Direction, MediaKind};
    use transport::{MediaPacket, TransportIncomingEvent};

    use crate::{
        rpc::WebrtcConnectRequestSender,
        transport::{internal::Str0mInput, mid_convert::track_to_mid},
    };

    use super::{life_cycle::sdk::SdkTransportLifeCycle, WebrtcTransportInternal};

    fn create_connected_internal() -> WebrtcTransportInternal<SdkTransportLifeCycle> {
        let mut internal = WebrtcTransportInternal::new(SdkTransportLifeCycle::new(0));

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
            kind: "audio".to_string(),
            name: "audio_main".to_string(),
            uuid: "track_id".to_string(),
            label: "label".to_string(),
            screen: None,
        });
        internal
            .on_str0m_event(100, Str0mInput::MediaAdded(Direction::RecvOnly, track_to_mid(100), MediaKind::Audio, None))
            .unwrap();

        assert_eq!(
            internal.endpoint_action(),
            Some(Ok(TransportIncomingEvent::RemoteTrackAdded(
                "audio_main".to_string(),
                100,
                transport::TrackMeta::from_kind(transport::MediaKind::Audio, Some("label".to_string())),
            )))
        );

        internal.on_str0m_event(100, Str0mInput::MediaChanged(Direction::Inactive, track_to_mid(100))).unwrap();

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
        let mut internal = WebrtcTransportInternal::new(SdkTransportLifeCycle::new(0));

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
        let mut internal = WebrtcTransportInternal::new(SdkTransportLifeCycle::new(0));

        internal
            .on_endpoint_event(
                10,
                transport::TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                    peer_hash: 1,
                    peer: "test".to_string(),
                    kind: transport::MediaKind::Audio,
                    state: None,
                    track: "track".to_string(),
                })),
            )
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
                "{\"data\":{\"kind\":\"audio\",\"peer\":\"test\",\"peer_hash\":1,\"state\":null,\"stream\":\"track\"},\"event\":\"stream_added\",\"type\":\"event\"}".to_string()
            ))
        );
    }

    #[test]
    fn simple_flow_webrtc_remote_pkt() {
        let mut internal = create_connected_internal();

        internal.map_remote_stream(WebrtcConnectRequestSender {
            kind: "audio".to_string(),
            name: "audio_main".to_string(),
            uuid: "track_id".to_string(),
            label: "label".to_string(),
            screen: None,
        });

        internal
            .on_str0m_event(1000, Str0mInput::MediaAdded(Direction::RecvOnly, track_to_mid(100), MediaKind::Audio, None))
            .unwrap();

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
