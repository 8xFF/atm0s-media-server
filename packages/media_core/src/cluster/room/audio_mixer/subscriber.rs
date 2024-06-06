use std::{collections::HashMap, fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::{
    features::pubsub::{self, ChannelId},
    NodeId,
};
use media_server_protocol::{
    endpoint::{AudioMixerPkt, PeerHashCode, PeerId},
    media::{MediaMeta, MediaPacket},
    transport::LocalTrackId,
};
use sans_io_runtime::{collections::DynamicDeque, return_if_none, TaskSwitcherChild};

use crate::cluster::{ClusterEndpointEvent, ClusterLocalTrackEvent};

use super::Output;

struct EndpointSlot {
    peer: PeerHashCode,
    tracks: Vec<LocalTrackId>,
}

pub struct AudioMixerSubscriber<Endpoint> {
    channel_id: ChannelId,
    queue: DynamicDeque<Output<Endpoint>, 16>,
    endpoints: HashMap<Endpoint, EndpointSlot>,
    mixer: audio_mixer::AudioMixer<(NodeId, u8)>,
}

impl<Endpoint: Debug + Hash + Eq + Clone> AudioMixerSubscriber<Endpoint> {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            channel_id,
            queue: Default::default(),
            endpoints: HashMap::new(),
            mixer: audio_mixer::AudioMixer::new(3), //TODO dynamic this
        }
    }

    pub fn on_tick(&mut self, now: u64) {
        if let Some(removed_slots) = self.mixer.on_tick(now) {}
    }

    pub fn on_endpoint_join(&mut self, endpoint: Endpoint, peer: PeerId, tracks: Vec<LocalTrackId>) {
        assert!(!self.endpoints.contains_key(&endpoint));
        log::info!("[AudioMixerSubsciber] endpoint {:?} peer {peer} join with tracks {:?}", endpoint, tracks);
        if self.endpoints.is_empty() {
            log::info!("[AudioMixerSubsciber] first endpoint join as Auto mode => subscribe channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::SubAuto)));
        }
        self.endpoints.insert(endpoint, EndpointSlot { peer: peer.hash_code(), tracks });
    }

    pub fn on_channel_data(&mut self, now: u64, from: NodeId, pkt: Vec<u8>) {
        let audio = return_if_none!(AudioMixerPkt::deserialize(&pkt));
        if let Some((slot, just_pinned)) = self.mixer.on_pkt(now, (from, audio.slot), audio.audio_level) {
            for (endpoint, endpoint_slot) in self.endpoints.iter() {
                if endpoint_slot.peer != audio.peer {
                    let track_id = endpoint_slot.tracks[slot];
                    if just_pinned {
                        self.queue.push_back(Output::Endpoint(
                            vec![endpoint.clone()],
                            ClusterEndpointEvent::LocalTrack(track_id, ClusterLocalTrackEvent::SourceChanged),
                        ));
                    }
                    self.queue.push_back(Output::Endpoint(
                        vec![endpoint.clone()],
                        ClusterEndpointEvent::LocalTrack(
                            track_id,
                            ClusterLocalTrackEvent::Media(
                                (audio.peer.0 << 16) | (audio.track.0 as u64), //TODO better track UUID
                                MediaPacket {
                                    ts: audio.ts,
                                    seq: audio.seq,
                                    marker: true,
                                    nackable: false,
                                    layers: None,
                                    meta: MediaMeta::Opus { audio_level: audio.audio_level },
                                    data: audio.opus_payload.clone(),
                                },
                            ),
                        ),
                    ))
                }
            }
        }
    }

    pub fn on_endpoint_leave(&mut self, endpoint: Endpoint) {
        assert!(self.endpoints.contains_key(&endpoint));
        log::info!("[AudioMixerSubsciber] endpoint {:?} leave", endpoint);
        self.endpoints.remove(&endpoint);
        if self.endpoints.is_empty() {
            log::info!("[AudioMixerSubsciber] last endpoint leave in Auto mode => unsubscribe channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::UnsubAuto)));
        }
    }
}

impl<Endpoint> TaskSwitcherChild<Output<Endpoint>> for AudioMixerSubscriber<Endpoint> {
    type Time = Instant;
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}
