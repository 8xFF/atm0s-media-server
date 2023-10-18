use std::collections::{HashMap, VecDeque};

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use str0m::{channel::ChannelId, media::Direction, Event};
use transport::{
    LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, MediaPacketExtensions, MediaSampleRate, RemoteTrackIncomingEvent, RemoteTrackOutgoingEvent, TrackId, TrackMeta, TransportError,
    TransportIncomingEvent, TransportOutgoingEvent, TransportRuntimeError,
};

use self::{
    life_cycle::{life_cycle_event_to_event, TransportLifeCycle},
    local_track_id_generator::LocalTrackIdGenerator,
    mid_history::MidHistory,
    msid_alias::MsidAlias,
    rpc::{rpc_from_string, rpc_local_track_to_string, rpc_remote_track_to_string, rpc_to_string, IncomingRpc},
    utils::{to_transport_kind, track_to_mid},
};
use crate::rpc::WebrtcConnectRequestSender;

use super::{Str0mAction, WebrtcTransportEvent};

pub(crate) mod life_cycle;
mod local_track_id_generator;
mod mid_history;
mod msid_alias;
mod rpc;
mod utils;

pub struct WebrtcTransportInternal<L>
where
    L: TransportLifeCycle,
{
    life_cycle: L,
    msid_alias: MsidAlias,
    mid_history: MidHistory,
    local_track_id_map: HashMap<String, TrackId>,
    remote_track_id_map: HashMap<String, TrackId>,
    local_track_id_gen: LocalTrackIdGenerator,
    channel_id: Option<ChannelId>,
    channel_pending_msgs: VecDeque<String>,
    endpoint_actions: VecDeque<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>>,
    str0m_actions: VecDeque<Str0mAction>,
}

impl<L> WebrtcTransportInternal<L>
where
    L: TransportLifeCycle,
{
    pub fn new(life_cycle: L) -> Self {
        log::info!("[TransportWebrtcInternal] created");

        Self {
            life_cycle,
            msid_alias: Default::default(),
            mid_history: Default::default(),
            local_track_id_map: Default::default(),
            remote_track_id_map: Default::default(),
            local_track_id_gen: Default::default(),
            channel_id: None,
            channel_pending_msgs: Default::default(),
            endpoint_actions: Default::default(),
            str0m_actions: Default::default(),
        }
    }

    pub fn map_remote_stream(&mut self, sender: WebrtcConnectRequestSender) {
        self.msid_alias.add_alias(&sender.uuid, &sender.label, &sender.kind, &sender.name);
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
                RemoteTrackOutgoingEvent::RequestKeyFrame => {
                    let mid = track_to_mid(track_id);
                    self.str0m_actions.push_back(Str0mAction::RequestKeyFrame(mid));
                }
                RemoteTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_remote_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on remote track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RequestLimitBitrate(_bitrate) => {
                //TODO
            }
            TransportOutgoingEvent::Rpc(rpc) => {
                let msg = rpc_to_string(rpc);
                log::info!("[TransportWebrtc] on endpoint out rpc: {}", msg);
                self.send_msg(msg);
            }
        }
        Ok(())
    }

    pub fn on_custom_event(&mut self, _now_ms: u64, _event: WebrtcTransportEvent) -> Result<(), TransportError> {
        Ok(())
    }

    pub fn on_str0m_event(&mut self, now_ms: u64, event: str0m::Event) -> Result<(), TransportError> {
        match event {
            Event::Connected => {
                life_cycle_event_to_event(self.life_cycle.on_webrtc_connected(now_ms), &mut self.endpoint_actions);
                Ok(())
            }
            Event::ChannelOpen(chanel_id, _name) => {
                self.channel_id = Some(chanel_id);
                self.restore_msgs();
                life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, true), &mut self.endpoint_actions);
                Ok(())
            }
            Event::ChannelData(data) => {
                if !data.binary {
                    if let Ok(data) = String::from_utf8(data.data) {
                        match rpc_from_string(&data) {
                            Ok(IncomingRpc::Endpoint(rpc)) => {
                                log::info!("[TransportWebrtcInternal] on incoming endpoint rpc: [{:?}]", rpc);
                                self.endpoint_actions.push_back(Ok(TransportIncomingEvent::Rpc(rpc)));
                                Ok(())
                            }
                            Ok(IncomingRpc::LocalTrack(track_name, rpc)) => {
                                if let Some(track_id) = self.local_track_id_map.get(&track_name) {
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
                                if let Some(track_id) = self.remote_track_id_map.get(&track_name) {
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
                } else {
                    Err(TransportError::RuntimeError(TransportRuntimeError::RpcInvalid))
                }
            }
            Event::ChannelClose(_chanel_id) => {
                life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, false), &mut self.endpoint_actions);
                Ok(())
            }
            Event::IceConnectionStateChange(state) => {
                life_cycle_event_to_event(self.life_cycle.on_ice_state(now_ms, state), &mut self.endpoint_actions);
                Ok(())
            }
            Event::RtpPacket(rtp) => {
                let track_id = rtp.header.ext_vals.mid.map(|mid| utils::mid_to_track(&mid));
                let ssrc: &u32 = &rtp.header.ssrc;
                if let Some(track_id) = self.mid_history.get(track_id, *ssrc) {
                    // log::info!("on rtp {} => {}", rtp.header.ssrc, track_id);
                    self.endpoint_actions.push_back(Ok(TransportIncomingEvent::RemoteTrackEvent(
                        track_id,
                        RemoteTrackIncomingEvent::MediaPacket(MediaPacket {
                            pt: *(&rtp.header.payload_type as &u8),
                            seq_no: rtp.header.sequence_number,
                            time: rtp.header.timestamp,
                            marker: rtp.header.marker,
                            ext_vals: MediaPacketExtensions {
                                abs_send_time: rtp.header.ext_vals.abs_send_time.map(|t| (t.numer(), t.denom())),
                                transport_cc: rtp.header.ext_vals.transport_cc,
                            },
                            nackable: true,
                            payload: rtp.payload,
                        }),
                    )));
                    Ok(())
                } else {
                    log::warn!("on rtp without mid {}", rtp.header.ssrc);
                    Err(TransportError::RuntimeError(TransportRuntimeError::TrackIdNotFound))
                }
            }
            Event::MediaAdded(added) => {
                match added.direction {
                    Direction::RecvOnly => {
                        //remote stream
                        let track_id = utils::mid_to_track(&added.mid);
                        if let Some(info) = self.msid_alias.get_alias(&added.msid.stream_id, &added.msid.track_id) {
                            self.remote_track_id_map.insert(info.name.clone(), track_id);
                            log::info!("[TransportWebrtcInternal] added remote track {} => {} added {:?} {:?}", info.name, track_id, added, info);
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::RemoteTrackAdded(
                                info.name,
                                track_id,
                                TrackMeta {
                                    kind: to_transport_kind(added.kind),
                                    sample_rate: MediaSampleRate::HzCustom(0), //TODO
                                    label: Some(info.label),
                                },
                            )));
                            Ok(())
                        } else {
                            Err(TransportError::RuntimeError(TransportRuntimeError::TrackIdNotFound))
                        }
                    }
                    Direction::SendOnly => {
                        //local stream
                        let track_id = utils::mid_to_track(&added.mid);
                        let track_name = self.local_track_id_gen.generate(added.kind, added.mid);
                        log::info!("[TransportWebrtcInternal] added local track {} => {} added {:?}", track_name, track_id, added);
                        self.local_track_id_map.insert(track_name.clone(), track_id);
                        self.endpoint_actions.push_back(Ok(TransportIncomingEvent::LocalTrackAdded(
                            track_name,
                            track_id,
                            TrackMeta {
                                kind: to_transport_kind(added.kind),
                                sample_rate: MediaSampleRate::HzCustom(0), //TODO
                                label: None,
                            },
                        )));
                        Ok(())
                    }
                    _ => {
                        panic!("not supported")
                    }
                }
            }
            Event::MediaChanged(_media) => {
                //TODO
                Ok(())
            }
            Event::StreamPaused(_paused) => {
                //TODO
                Ok(())
            }
            Event::KeyframeRequest(req) => {
                let track_id = utils::mid_to_track(&req.mid);
                self.endpoint_actions
                    .push_back(Ok(TransportIncomingEvent::LocalTrackEvent(track_id, LocalTrackIncomingEvent::RequestKeyFrame)));
                Ok(())
            }
            _ => Ok(()),
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
    use std::time::Instant;

    use endpoint::{rpc::TrackInfo, EndpointRpcOut};
    use str0m::{
        channel::{ChannelData, ChannelId},
        media::{MediaAdded, MediaTime},
        rtp::{RtpHeader, RtpPacket},
        Msid,
    };
    use transport::TransportIncomingEvent;

    use crate::rpc::WebrtcConnectRequestSender;

    use super::{life_cycle::sdk::SdkTransportLifeCycle, utils::track_to_mid, WebrtcTransportInternal};

    fn create_connected_internal() -> WebrtcTransportInternal<SdkTransportLifeCycle> {
        let mut internal = WebrtcTransportInternal::new(SdkTransportLifeCycle::new(0));

        // we need wait both webrtc and datachannel connected
        internal.on_str0m_event(100, str0m::Event::Connected).unwrap();
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);
        internal.on_str0m_event(100, str0m::Event::ChannelOpen(ChannelId(0), "data".to_string())).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Connected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal
    }

    #[test]
    fn simple_flow_webrtc_connected() {
        let mut internal = create_connected_internal();

        internal.on_str0m_event(100, str0m::Event::ChannelClose(ChannelId(0))).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Disconnected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);
    }

    #[test]
    fn simple_flow_webrtc_connection_reconnect() {
        let mut internal = create_connected_internal();

        internal.on_str0m_event(100, str0m::Event::IceConnectionStateChange(str0m::IceConnectionState::Disconnected)).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Reconnecting))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.on_str0m_event(100, str0m::Event::IceConnectionStateChange(str0m::IceConnectionState::Connected)).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Reconnected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);
    }

    #[test]
    fn simple_flow_webrtc_connection_failed() {
        let mut internal = create_connected_internal();

        internal.on_str0m_event(100, str0m::Event::IceConnectionStateChange(str0m::IceConnectionState::Disconnected)).unwrap();
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

        let data = ChannelData {
            id: ChannelId(0),
            binary: false,
            data: "".as_bytes().to_vec(),
        };
        assert_eq!(
            internal.on_str0m_event(1000, str0m::Event::ChannelData(data)),
            Err(transport::TransportError::RuntimeError(transport::TransportRuntimeError::RpcInvalid))
        );
    }

    #[test]
    fn should_fire_rpc() {
        let mut internal = create_connected_internal();

        let req_str = r#"{"req_id":1,"type":"request","request":"peer.close"}"#;
        let data = ChannelData {
            id: ChannelId(0),
            binary: false,
            data: req_str.as_bytes().to_vec(),
        };

        assert_eq!(internal.on_str0m_event(1000, str0m::Event::ChannelData(data)), Ok(()));
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
        internal.on_str0m_event(100, str0m::Event::Connected).unwrap();
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(internal.str0m_action(), None);

        internal.on_str0m_event(100, str0m::Event::ChannelOpen(ChannelId(0), "data".to_string())).unwrap();
        assert_eq!(internal.endpoint_action(), Some(Ok(TransportIncomingEvent::State(transport::TransportStateEvent::Connected))));
        assert_eq!(internal.endpoint_action(), None);
        assert_eq!(
            internal.str0m_action(),
            Some(crate::transport::Str0mAction::Datachannel(
                ChannelId(0),
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
            .on_str0m_event(
                1000,
                str0m::Event::MediaAdded(MediaAdded {
                    mid: track_to_mid(100),
                    msid: Msid {
                        stream_id: "stream_id".to_string(),
                        track_id: "track_id".to_string(),
                    },
                    kind: str0m::media::MediaKind::Audio,
                    direction: str0m::media::Direction::RecvOnly,
                    simulcast: None,
                }),
            )
            .unwrap();

        // must fire remote track added to endpoint
        assert_eq!(
            internal.endpoint_action(),
            Some(Ok(TransportIncomingEvent::RemoteTrackAdded(
                "audio_main".to_string(),
                100,
                transport::TrackMeta {
                    kind: transport::MediaKind::Audio,
                    sample_rate: transport::MediaSampleRate::HzCustom(0),
                    label: Some("label".to_string()),
                }
            )))
        );
        assert_eq!(internal.endpoint_action(), None);

        // must fire remote track pkt to endpoint
        {
            let mut header = RtpHeader::default();
            header.ext_vals.mid = Some(track_to_mid(100));
            header.ssrc = 10000.into();
            header.payload_type = 1.into();
            header.sequence_number = 1;
            header.timestamp = 1000;

            let pkt = RtpPacket {
                seq_no: 1.into(),
                time: MediaTime::new(1000, 1000),
                header,
                payload: vec![1, 2, 3],
                timestamp: Instant::now(),
                last_sender_info: None,
                nackable: false,
            };
            internal.on_str0m_event(2000, str0m::Event::RtpPacket(pkt)).unwrap();

            assert_eq!(
                internal.endpoint_action(),
                Some(Ok(TransportIncomingEvent::RemoteTrackEvent(
                    100,
                    transport::RemoteTrackIncomingEvent::MediaPacket(transport::MediaPacket {
                        pt: 1,
                        seq_no: 1,
                        time: 1000,
                        marker: false,
                        ext_vals: transport::MediaPacketExtensions {
                            abs_send_time: None,
                            transport_cc: None,
                        },
                        nackable: true,
                        payload: vec![1, 2, 3],
                    })
                )))
            );
        }

        // must fire remote track rtp without mid to endpoint
        {
            let mut header = RtpHeader::default();
            header.ssrc = 10000.into();
            header.payload_type = 1.into();
            header.sequence_number = 2;
            header.timestamp = 1000;

            let pkt = RtpPacket {
                seq_no: 2.into(),
                time: MediaTime::new(1000, 1000),
                header,
                payload: vec![1, 2, 3],
                timestamp: Instant::now(),
                last_sender_info: None,
                nackable: false,
            };
            internal.on_str0m_event(2000, str0m::Event::RtpPacket(pkt)).unwrap();

            assert_eq!(
                internal.endpoint_action(),
                Some(Ok(TransportIncomingEvent::RemoteTrackEvent(
                    100,
                    transport::RemoteTrackIncomingEvent::MediaPacket(transport::MediaPacket {
                        pt: 1,
                        seq_no: 2,
                        time: 1000,
                        marker: false,
                        ext_vals: transport::MediaPacketExtensions {
                            abs_send_time: None,
                            transport_cc: None,
                        },
                        nackable: true,
                        payload: vec![1, 2, 3],
                    })
                )))
            );
        }
    }
}
