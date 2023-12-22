use std::{collections::VecDeque, vec};

use audio_mixer::{AudioMixer, AudioMixerOutput};
use cluster::{ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent, ClusterTrackUuid, MixMinusAudioMode};
use media_utils::{SeqRewrite, TsRewrite};
use transport::{LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaKind, MediaPacket, TrackId, TransportError, TransportIncomingEvent, TransportOutgoingEvent};

use crate::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, TrackInfo},
    EndpointRpcIn, EndpointRpcOut, MediaEndpointMiddleware, MediaEndpointMiddlewareOutput, RpcResponse,
};

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
    name: String,
    mode: MixMinusAudioMode,
    mixer: AudioMixer<MediaPacket, ClusterTrackUuid>,
    output_slots: Vec<Slot>,
    outputs: VecDeque<MediaEndpointMiddlewareOutput>,
}

impl MixMinusEndpointMiddleware {
    pub fn new(room: &str, name: &str, mode: MixMinusAudioMode, virtual_track_id: u16, outputs: usize) -> Self {
        Self {
            virtual_track_id,
            room: room.to_string(),
            name: name.to_string(),
            mode,
            mixer: AudioMixer::new(Box::new(|pkt| pkt.ext_vals.audio_level), audio_mixer::AudioMixerConfig { outputs }),
            output_slots: vec![Default::default(); outputs],
            outputs: VecDeque::new(),
        }
    }
}

impl MediaEndpointMiddleware for MixMinusEndpointMiddleware {
    fn on_start(&mut self, _now_ms: u64) {
        for i in 0..self.output_slots.len() {
            self.outputs
                .push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                    peer_hash: 0,
                    peer: "".to_string(),
                    kind: MediaKind::Audio,
                    track: format!("mix_minus_{}_{}", self.name, i),
                    state: None,
                }))));
        }
    }

    fn on_tick(&mut self, now_ms: u64) {
        self.mixer.on_tick(now_ms);
    }

    fn on_transport(&mut self, now_ms: u64, event: &TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>) -> bool {
        match event {
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
                    let remote_peer = req.data.remote.peer.clone();
                    let remote_stream = req.data.remote.stream.clone();
                    self.mixer.add_source(now_ms, ClusterTrackUuid::from_info(&self.room, &remote_peer, &remote_stream));
                    self.outputs
                        .push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::MixMinusSourceAddRes(
                            RpcResponse::success(req.req_id, true),
                        ))));
                    self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                        self.virtual_track_id,
                        ClusterLocalTrackOutgoingEvent::Subscribe(remote_peer, remote_stream),
                    )));
                    true
                } else {
                    false
                }
            }
            TransportIncomingEvent::Rpc(EndpointRpcIn::MixMinusSourceRemove(req)) => {
                if matches!(self.mode, MixMinusAudioMode::ManualAudioStreams) && req.data.id == self.name {
                    let remote_peer = req.data.remote.peer.clone();
                    let remote_stream = req.data.remote.stream.clone();
                    self.mixer.remove_source(now_ms, ClusterTrackUuid::from_info(&self.room, &remote_peer, &remote_stream));
                    self.outputs
                        .push_back(MediaEndpointMiddlewareOutput::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::MixMinusSourceRemoveRes(
                            RpcResponse::success(req.req_id, true),
                        ))));
                    self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                        self.virtual_track_id,
                        ClusterLocalTrackOutgoingEvent::Unsubscribe(remote_peer, remote_stream),
                    )));
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
            ClusterEndpointIncomingEvent::PeerTrackAdded(peer, track, _meta) => {
                if matches!(self.mode, MixMinusAudioMode::AllAudioStreams) {
                    self.mixer.add_source(now_ms, ClusterTrackUuid::from_info(&self.room, peer, track));
                    self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                        self.virtual_track_id,
                        ClusterLocalTrackOutgoingEvent::Subscribe(peer.clone(), track.clone()),
                    )));
                }
                false
            }
            ClusterEndpointIncomingEvent::PeerTrackRemoved(peer, track) => {
                if matches!(self.mode, MixMinusAudioMode::AllAudioStreams) {
                    self.mixer.remove_source(now_ms, ClusterTrackUuid::from_info(&self.room, peer, track));
                    self.outputs.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                        self.virtual_track_id,
                        ClusterLocalTrackOutgoingEvent::Unsubscribe(peer.clone(), track.clone()),
                    )));
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
                AudioMixerOutput::OutputSlotSrcChanged(slot, _) => {
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
}

//TODO test
