use std::collections::{HashMap, VecDeque};

use audio_mixer::{AudioMixer, AudioMixerOutput};
use cluster::{ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent, ClusterTrackUuid, MixMinusAudioMode};
use media_utils::{SeqRewrite, TsRewrite};
use transport::{LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, TrackId, TransportError, TransportIncomingEvent, TransportOutgoingEvent, TransportStateEvent};

use crate::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn},
    EndpointRpcIn, EndpointRpcOut, RpcResponse,
};

use super::{MediaEndpointMiddleware, MediaEndpointMiddlewareOutput};

const SEQ_MAX: u64 = 1 << 16;
const TS_MAX: u64 = 1 << 32;
const AUDIO_SAMPLE_RATE: u64 = 48000;

pub type MediaSeqRewrite = SeqRewrite<SEQ_MAX, 1000>;
pub type MediaTsRewrite = TsRewrite<TS_MAX, 10>;

#[derive(Clone)]
struct Slot {
    track_id: Option<TrackId>,
    ts_rewritter: MediaTsRewrite,
    seq_rewriter: MediaSeqRewrite,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            track_id: None,
            ts_rewritter: MediaTsRewrite::new(AUDIO_SAMPLE_RATE),
            seq_rewriter: MediaSeqRewrite::default(),
        }
    }
}

pub struct MixMinusEndpointMiddleware {
    virtual_track_id: u16,
    room: String,
    peer: String,
    name: String,
    mode: MixMinusAudioMode,
    mixer: AudioMixer<MediaPacket, ClusterTrackUuid>,
    output_slots: Vec<Slot>,
    outputs: VecDeque<MediaEndpointMiddlewareOutput>,
    current_subs: HashMap<(String, String), ()>,
    connected: bool,
}

impl MixMinusEndpointMiddleware {
    pub fn new(room: &str, peer: &str, name: &str, mode: MixMinusAudioMode, virtual_track_id: u16, outputs: &[Option<TrackId>]) -> Self {
        Self {
            virtual_track_id,
            room: room.to_string(),
            peer: peer.to_string(),
            name: name.to_string(),
            mode,
            mixer: AudioMixer::new(Box::new(|pkt| pkt.ext_vals.audio_level), audio_mixer::AudioMixerConfig { outputs: outputs.len() }),
            output_slots: outputs
                .iter()
                .map(|track_id| Slot {
                    track_id: track_id.clone(),
                    ..Default::default()
                })
                .collect(),
            outputs: VecDeque::new(),
            current_subs: HashMap::new(),
            connected: false,
        }
    }

    fn add_source(&mut self, now_ms: u64, peer: &str, track: &str) {
        if self.mixer.add_source(now_ms, ClusterTrackUuid::from_info(&self.room, peer, track)).is_some() {
            self.current_subs.insert((peer.to_string(), track.to_string()), ());
            if self.connected {
                self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                    self.virtual_track_id,
                    ClusterLocalTrackOutgoingEvent::Subscribe(peer.to_string(), track.to_string()),
                )));
            }
        }
    }

    fn remove_source(&mut self, now_ms: u64, peer: &str, track: &str) {
        if self.mixer.remove_source(now_ms, ClusterTrackUuid::from_info(&self.room, peer, track)).is_some() {
            self.current_subs.remove(&(peer.to_string(), track.to_string()));
            if self.connected {
                self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                    self.virtual_track_id,
                    ClusterLocalTrackOutgoingEvent::Unsubscribe(peer.to_string(), track.to_string()),
                )));
            }
        }
    }
}

impl MediaEndpointMiddleware for MixMinusEndpointMiddleware {
    fn on_start(&mut self, _now_ms: u64) {
        // current version sdk dont need to fire event, it auto subscribe to mix_minus_default_0,1,2
    }

    fn on_tick(&mut self, now_ms: u64) {
        self.mixer.on_tick(now_ms);
    }

    fn on_transport(&mut self, now_ms: u64, event: &TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>) -> bool {
        match event {
            TransportIncomingEvent::State(state) => {
                if state == &TransportStateEvent::Connected {
                    self.connected = true;
                    for (peer, track) in self.current_subs.keys() {
                        self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                            self.virtual_track_id,
                            ClusterLocalTrackOutgoingEvent::Subscribe(peer.to_string(), track.to_string()),
                        )));
                    }
                }
                false
            }
            TransportIncomingEvent::LocalTrackEvent(track_id, LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(req))) => {
                if req.data.remote.peer.is_empty() && req.data.remote.stream.starts_with(&format!("mix_minus_{}_", self.name)) {
                    //extract slot_index from mix_minus_{name}_slot_{index}
                    let res = if let Some(Ok(slot_index)) = req.data.remote.stream.split('_').last().map(|i| i.parse::<usize>()) {
                        if let Some(slot) = self.output_slots.get_mut(slot_index) {
                            slot.track_id.replace(*track_id);
                            RpcResponse::success(req.req_id, true)
                        } else {
                            RpcResponse::error(req.req_id)
                        }
                    } else {
                        RpcResponse::error(req.req_id)
                    };

                    self.outputs.push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                        *track_id,
                        LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(res)),
                    )));

                    true
                } else {
                    false
                }
            }
            TransportIncomingEvent::LocalTrackEvent(track_id, LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Disconnect(req))) => {
                let track_id = *track_id;
                if let Some(found_slot) = self.output_slots.iter_mut().find(move |slot| Some(track_id).eq(&slot.track_id)) {
                    found_slot.track_id = None;
                    self.outputs.push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                        track_id,
                        LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::DisconnectRes(RpcResponse::success(req.req_id, true))),
                    )));
                    true
                } else {
                    false
                }
            }
            TransportIncomingEvent::Rpc(EndpointRpcIn::MixMinusSourceAdd(req)) => {
                if matches!(self.mode, MixMinusAudioMode::ManualAudioStreams) && req.data.id == self.name {
                    self.outputs
                        .push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::MixMinusSourceAddRes(
                            RpcResponse::success(req.req_id, true),
                        ))));
                    self.add_source(now_ms, &req.data.remote.peer, &req.data.remote.stream);
                    true
                } else {
                    false
                }
            }
            TransportIncomingEvent::Rpc(EndpointRpcIn::MixMinusSourceRemove(req)) => {
                if matches!(self.mode, MixMinusAudioMode::ManualAudioStreams) && req.data.id == self.name {
                    self.outputs
                        .push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::MixMinusSourceRemoveRes(
                            RpcResponse::success(req.req_id, true),
                        ))));
                    self.remove_source(now_ms, &req.data.remote.peer, &req.data.remote.stream);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn on_transport_error(&mut self, _now_ms: u64, _error: &TransportError) -> bool {
        false
    }

    fn on_cluster(&mut self, now_ms: u64, event: &ClusterEndpointIncomingEvent) -> bool {
        match event {
            ClusterEndpointIncomingEvent::PeerTrackAdded(peer, track, meta) => {
                if peer.eq(&self.peer) {
                    return false;
                }
                if matches!(self.mode, MixMinusAudioMode::AllAudioStreams) && meta.kind.is_audio() {
                    self.add_source(now_ms, peer, track);
                }
                false
            }
            ClusterEndpointIncomingEvent::PeerTrackRemoved(peer, track) => {
                if peer.eq(&self.peer) {
                    return false;
                }
                if matches!(self.mode, MixMinusAudioMode::AllAudioStreams) {
                    self.remove_source(now_ms, peer, track);
                }
                false
            }
            ClusterEndpointIncomingEvent::LocalTrackEvent(track, event) => {
                if *track == self.virtual_track_id {
                    if let ClusterLocalTrackIncomingEvent::MediaPacket(track_uuid, pkt) = event {
                        self.mixer.push_pkt(now_ms, *track_uuid, pkt);
                    }
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn pop_action(&mut self, now_ms: u64) -> Option<MediaEndpointMiddlewareOutput> {
        while let Some(out) = self.mixer.pop() {
            match out {
                AudioMixerOutput::SlotPinned(_, _) => {
                    //TODO fire event to client
                }
                AudioMixerOutput::SlotUnpinned(_, _) => {
                    //TODO fire event to client
                }
                AudioMixerOutput::OutputSlotSrcChanged(slot, src) => {
                    log::info!("[AudioMixMinus] slot {} changed to {:?}", slot, src);
                    if let Some(slot) = self.output_slots.get_mut(slot) {
                        slot.ts_rewritter.reinit();
                        slot.seq_rewriter.reinit();
                    }
                }
                AudioMixerOutput::OutputSlotPkt(slot, mut pkt) => {
                    if let Some(slot) = self.output_slots.get_mut(slot) {
                        if let Some(track_id) = slot.track_id {
                            if let Some(seq) = slot.seq_rewriter.generate(pkt.seq_no as u64) {
                                let ts = slot.ts_rewritter.generate(now_ms, pkt.time as u64);
                                pkt.time = ts as u32;
                                pkt.seq_no = seq as u16;
                                self.outputs.push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                                    track_id,
                                    LocalTrackOutgoingEvent::MediaPacket(pkt),
                                )));
                            }
                        }
                    }
                }
            }
        }

        self.outputs.pop_front()
    }

    fn before_drop(&mut self, _now_ms: u64) {
        let current_subs = std::mem::take(&mut self.current_subs);
        for (peer, track) in current_subs.into_keys() {
            self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                self.virtual_track_id,
                ClusterLocalTrackOutgoingEvent::Unsubscribe(peer, track),
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use cluster::{
        ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent, ClusterTrackMeta, ClusterTrackScalingType, ClusterTrackStatus,
        ClusterTrackUuid, MixMinusAudioMode,
    };
    use transport::{LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaKind, MediaPacket, TransportIncomingEvent, TransportOutgoingEvent, TransportStateEvent};

    use crate::{
        endpoint::middleware::{MediaEndpointMiddleware, MediaEndpointMiddlewareOutput},
        rpc::{LocalTrackRpcIn, LocalTrackRpcOut, MixMinusSource, ReceiverDisconnect, ReceiverSwitch, RemoteStream},
        EndpointRpcIn, EndpointRpcOut, RpcRequest, RpcResponse,
    };

    use super::MixMinusEndpointMiddleware;

    #[test]
    fn same_user_should_not_handle() {
        let mut mix_minus = MixMinusEndpointMiddleware::new("demo", "user0", "default", MixMinusAudioMode::AllAudioStreams, 100, &[None, None, None]);
        mix_minus.on_start(0);
        mix_minus.on_transport(0, &TransportIncomingEvent::State(TransportStateEvent::Connected));
        assert_eq!(mix_minus.pop_action(0), None);

        //should auto subscribe if cluster audio track added
        assert_eq!(
            mix_minus.on_cluster(
                0,
                &ClusterEndpointIncomingEvent::PeerTrackAdded("user0".to_string(), "audio_main".to_string(), ClusterTrackMeta::default_audio())
            ),
            false
        );

        assert_eq!(mix_minus.pop_action(0), None);
    }

    /// This test for ensuring predefined subscribe over outputs slice will be used
    #[test]
    fn pre_config_subscribe() {
        let virtual_track_id = 100;
        let local_track_id = 10;
        let mut mix_minus = MixMinusEndpointMiddleware::new("demo", "user0", "default", MixMinusAudioMode::AllAudioStreams, virtual_track_id, &[Some(local_track_id)]);
        mix_minus.on_start(0);
        mix_minus.on_transport(0, &TransportIncomingEvent::State(TransportStateEvent::Connected));
        assert_eq!(mix_minus.pop_action(0), None);

        //should auto subscribe if cluster audio track added
        assert_eq!(
            mix_minus.on_cluster(
                0,
                &ClusterEndpointIncomingEvent::PeerTrackAdded("user1".to_string(), "audio_main".to_string(), ClusterTrackMeta::default_audio())
            ),
            false
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                virtual_track_id,
                ClusterLocalTrackOutgoingEvent::Subscribe("user1".to_string(), "audio_main".to_string()),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);

        let user1_audio_uuid = ClusterTrackUuid::from_info("demo", "user1", "audio_main");
        let mut user1_pkt = MediaPacket::simple_audio(1, 1000, vec![1]);
        user1_pkt.ext_vals.audio_level = Some(50);

        mix_minus.on_cluster(
            0,
            &ClusterEndpointIncomingEvent::LocalTrackEvent(virtual_track_id, ClusterLocalTrackIncomingEvent::MediaPacket(user1_audio_uuid, user1_pkt)),
        );

        let mut desired_pkt = MediaPacket::simple_audio(1, 0, vec![1]);
        desired_pkt.ext_vals.audio_level = Some(50);

        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                local_track_id,
                LocalTrackOutgoingEvent::MediaPacket(desired_pkt),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);
    }

    #[test]
    fn track_sub_should_wait_connected() {
        let mut mix_minus = MixMinusEndpointMiddleware::new("demo", "user0", "default", MixMinusAudioMode::AllAudioStreams, 100, &[None, None, None]);
        mix_minus.on_start(0);
        assert_eq!(mix_minus.pop_action(0), None);

        //should not subscribe because transport not connected
        assert_eq!(
            mix_minus.on_cluster(
                0,
                &ClusterEndpointIncomingEvent::PeerTrackAdded("user1".to_string(), "audio_main".to_string(), ClusterTrackMeta::default_audio())
            ),
            false
        );
        assert_eq!(mix_minus.pop_action(0), None);

        //now connected then it should subscribe
        mix_minus.on_transport(0, &TransportIncomingEvent::State(TransportStateEvent::Connected));

        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                100,
                ClusterLocalTrackOutgoingEvent::Subscribe("user1".to_string(), "audio_main".to_string()),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);
    }

    #[test]
    fn handle_track_and_view() {
        let mut mix_minus = MixMinusEndpointMiddleware::new("demo", "user0", "default", MixMinusAudioMode::AllAudioStreams, 100, &[None, None, None]);
        mix_minus.on_start(0);
        mix_minus.on_transport(0, &TransportIncomingEvent::State(TransportStateEvent::Connected));
        assert_eq!(mix_minus.pop_action(0), None);

        //should auto subscribe if cluster audio track added
        assert_eq!(
            mix_minus.on_cluster(
                0,
                &ClusterEndpointIncomingEvent::PeerTrackAdded("user1".to_string(), "audio_main".to_string(), ClusterTrackMeta::default_audio())
            ),
            false
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                100,
                ClusterLocalTrackOutgoingEvent::Subscribe("user1".to_string(), "audio_main".to_string()),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);

        //should not subscribe if cluster video track added
        assert_eq!(
            mix_minus.on_cluster(
                0,
                &ClusterEndpointIncomingEvent::PeerTrackAdded("user1".to_string(), "video_main".to_string(), ClusterTrackMeta::default_video())
            ),
            false
        );
        assert_eq!(mix_minus.pop_action(0), None);

        //should not handle view event if dest is not mix_minus_default
        let event = RpcRequest {
            req_id: 0,
            data: ReceiverSwitch {
                id: "track_0".to_string(),
                priority: 100,
                remote: RemoteStream {
                    peer: "".to_string(),
                    stream: "mix_minus_other_0".to_string(),
                },
            },
        };
        assert_eq!(
            mix_minus.on_transport(0, &TransportIncomingEvent::LocalTrackEvent(1, LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(event)))),
            false
        );

        //should handle view event if dest is mix_minus_default
        let event = RpcRequest {
            req_id: 0,
            data: ReceiverSwitch {
                id: "track_0".to_string(),
                priority: 100,
                remote: RemoteStream {
                    peer: "".to_string(),
                    stream: "mix_minus_default_0".to_string(),
                },
            },
        };
        assert_eq!(
            mix_minus.on_transport(0, &TransportIncomingEvent::LocalTrackEvent(1, LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(event)))),
            true
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                1,
                LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(0, true))),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);

        //should handle disconnect
        let event = RpcRequest {
            req_id: 1,
            data: ReceiverDisconnect { id: "track_0".to_string() },
        };
        assert_eq!(
            mix_minus.on_transport(0, &TransportIncomingEvent::LocalTrackEvent(1, LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Disconnect(event)))),
            true
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                1,
                LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::DisconnectRes(RpcResponse::success(1, true))),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);
    }

    #[test]
    fn handle_manual_mode() {
        let mut mix_minus = MixMinusEndpointMiddleware::new("demo", "user0", "default", MixMinusAudioMode::ManualAudioStreams, 100, &[None, None, None]);
        mix_minus.on_start(0);
        mix_minus.on_transport(0, &TransportIncomingEvent::State(TransportStateEvent::Connected));

        // assert!(mix_minus.pop_action(0).is_some()); //track added
        // assert!(mix_minus.pop_action(0).is_some()); //track added
        // assert!(mix_minus.pop_action(0).is_some()); //track added
        assert_eq!(mix_minus.pop_action(0), None);

        //should not auto subscribe if remote track added
        assert_eq!(
            mix_minus.on_cluster(
                0,
                &ClusterEndpointIncomingEvent::PeerTrackAdded("user1".to_string(), "audio_main".to_string(), ClusterTrackMeta::default_audio())
            ),
            false
        );
        assert_eq!(mix_minus.pop_action(0), None);

        //should subscribe if request rpc add source
        assert_eq!(
            mix_minus.on_transport(
                0,
                &TransportIncomingEvent::Rpc(EndpointRpcIn::MixMinusSourceAdd(RpcRequest {
                    req_id: 0,
                    data: MixMinusSource {
                        id: "default".to_string(),
                        remote: RemoteStream {
                            peer: "user1".to_string(),
                            stream: "audio_main".to_string(),
                        },
                    },
                }))
            ),
            true
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::MixMinusSourceAddRes(
                RpcResponse::success(0, true)
            ))))
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                100,
                ClusterLocalTrackOutgoingEvent::Subscribe("user1".to_string(), "audio_main".to_string()),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);

        //should unsubscribe if request rpc remove source
        assert_eq!(
            mix_minus.on_transport(
                0,
                &TransportIncomingEvent::Rpc(EndpointRpcIn::MixMinusSourceRemove(RpcRequest {
                    req_id: 1,
                    data: MixMinusSource {
                        id: "default".to_string(),
                        remote: RemoteStream {
                            peer: "user1".to_string(),
                            stream: "audio_main".to_string(),
                        },
                    },
                }))
            ),
            true
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::MixMinusSourceRemoveRes(
                RpcResponse::success(1, true)
            ))))
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                100,
                ClusterLocalTrackOutgoingEvent::Unsubscribe("user1".to_string(), "audio_main".to_string()),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);
    }

    #[test]
    fn should_continuos_pkt_seq_ts_when_switch_source() {
        let mut mix_minus = MixMinusEndpointMiddleware::new("demo", "user0", "default", MixMinusAudioMode::AllAudioStreams, 100, &[None]);
        mix_minus.on_start(0);
        mix_minus.on_transport(0, &TransportIncomingEvent::State(TransportStateEvent::Connected));

        // assert!(mix_minus.pop_action(0).is_some()); //track added
        assert_eq!(mix_minus.pop_action(0), None);

        let meta = ClusterTrackMeta {
            active: true,
            kind: MediaKind::Audio,
            label: None,
            layers: vec![],
            scaling: ClusterTrackScalingType::Single,
            status: ClusterTrackStatus::Connected,
        };

        let user1_audio_uuid = ClusterTrackUuid::from_info("demo", "user1", "audio_main");
        let user2_audio_uuid = ClusterTrackUuid::from_info("demo", "user2", "audio_main");
        mix_minus.on_cluster(0, &ClusterEndpointIncomingEvent::PeerTrackAdded("user1".to_string(), "audio_main".to_string(), meta.clone()));
        mix_minus.on_cluster(0, &ClusterEndpointIncomingEvent::PeerTrackAdded("user2".to_string(), "audio_main".to_string(), meta.clone()));
        assert!(mix_minus.pop_action(0).is_some()); //track subscribe
        assert!(mix_minus.pop_action(0).is_some()); //track subscribe
        assert_eq!(mix_minus.pop_action(0), None);

        //should handle view event if dest is mix_minus_default
        let event = RpcRequest {
            req_id: 0,
            data: ReceiverSwitch {
                id: "track_0".to_string(),
                priority: 100,
                remote: RemoteStream {
                    peer: "".to_string(),
                    stream: "mix_minus_default_0".to_string(),
                },
            },
        };
        assert_eq!(
            mix_minus.on_transport(0, &TransportIncomingEvent::LocalTrackEvent(1, LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(event)))),
            true
        );
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                1,
                LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(0, true))),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);

        // should send pkt from user1 and not from user2
        let mut user1_pkt = MediaPacket::simple_audio(1, 1000, vec![1]);
        user1_pkt.ext_vals.audio_level = Some(50);

        let mut user2_pkt = MediaPacket::simple_audio(1000, 5000, vec![2]);
        user2_pkt.ext_vals.audio_level = Some(50);

        mix_minus.on_cluster(
            0,
            &ClusterEndpointIncomingEvent::LocalTrackEvent(100, ClusterLocalTrackIncomingEvent::MediaPacket(user1_audio_uuid, user1_pkt)),
        );
        mix_minus.on_cluster(
            0,
            &ClusterEndpointIncomingEvent::LocalTrackEvent(100, ClusterLocalTrackIncomingEvent::MediaPacket(user2_audio_uuid, user2_pkt)),
        );

        //shoukd pop pkt from user1
        let mut desired_pkt = MediaPacket::simple_audio(1, 0, vec![1]);
        desired_pkt.ext_vals.audio_level = Some(50);
        assert_eq!(
            mix_minus.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                1,
                LocalTrackOutgoingEvent::MediaPacket(desired_pkt),
            )))
        );
        assert_eq!(mix_minus.pop_action(0), None);

        // after user2 have higher audio_level should switch to user2
        let mut user2_pkt = MediaPacket::simple_audio(1001, 5020, vec![2, 3]);
        user2_pkt.ext_vals.audio_level = Some(100);
        mix_minus.on_cluster(
            20,
            &ClusterEndpointIncomingEvent::LocalTrackEvent(100, ClusterLocalTrackIncomingEvent::MediaPacket(user2_audio_uuid, user2_pkt)),
        );

        //shoukd pop pkt from user2
        let mut desired_pkt = MediaPacket::simple_audio(2, 960, vec![2, 3]);
        desired_pkt.ext_vals.audio_level = Some(100);
        assert_eq!(
            mix_minus.pop_action(20),
            Some(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                1,
                LocalTrackOutgoingEvent::MediaPacket(desired_pkt),
            )))
        );
        assert_eq!(mix_minus.pop_action(20), None);
    }
}
