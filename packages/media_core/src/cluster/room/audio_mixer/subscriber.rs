use std::{array, fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::{
    features::pubsub::{self, ChannelId},
    NodeId,
};
use indexmap::IndexMap;
use media_server_protocol::{
    endpoint::{AudioMixerPkt, PeerHashCode, PeerId, TrackName},
    media::{MediaMeta, MediaPacket},
    transport::LocalTrackId,
};
use sans_io_runtime::{collections::DynamicDeque, return_if_none, TaskSwitcherChild};

use crate::cluster::{ClusterAudioMixerEvent, ClusterEndpointEvent, ClusterLocalTrackEvent};

use super::Output;

struct EndpointSlot {
    peer: PeerHashCode,
    tracks: Vec<LocalTrackId>,
}

#[derive(Clone)]
struct OutputSlot {
    source: Option<(PeerId, TrackName)>,
}

pub struct AudioMixerSubscriber<Endpoint, const OUTPUTS: usize> {
    channel_id: ChannelId,
    queue: DynamicDeque<Output<Endpoint>, 16>,
    endpoints: IndexMap<Endpoint, EndpointSlot>,
    outputs: [Option<OutputSlot>; OUTPUTS],
    mixer: audio_mixer::AudioMixer<(NodeId, u8)>,
}

impl<Endpoint: Debug + Hash + Eq + Clone, const OUTPUTS: usize> AudioMixerSubscriber<Endpoint, OUTPUTS> {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            channel_id,
            queue: Default::default(),
            endpoints: IndexMap::new(),
            outputs: array::from_fn(|_| None),
            mixer: audio_mixer::AudioMixer::new(OUTPUTS),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty() && self.queue.is_empty()
    }

    pub fn on_tick(&mut self, now: Instant) {
        if let Some(removed_slots) = self.mixer.on_tick(now) {
            for slot in removed_slots {
                self.outputs[slot] = None;
                for (endpoint, _) in self.endpoints.iter() {
                    self.queue.push_back(Output::Endpoint(
                        vec![endpoint.clone()],
                        ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotUnset(slot as u8)),
                    ));
                }
            }
        }
    }

    /// We a endpoint join we need to restore current set slots
    pub fn on_endpoint_join(&mut self, _now: Instant, endpoint: Endpoint, peer: PeerId, tracks: Vec<LocalTrackId>) {
        assert!(!self.endpoints.contains_key(&endpoint));
        log::info!("[ClusterAudioMixerSubscriber {OUTPUTS}] endpoint {:?} peer {peer} join with tracks {:?}", endpoint, tracks);
        if self.endpoints.is_empty() {
            log::info!("[ClusterAudioMixerSubscriber {OUTPUTS}] first endpoint join as Auto mode => subscribe channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::SubAuto)));
        }

        for (index, slot) in self.outputs.iter().enumerate() {
            if let Some(slot) = slot {
                if let Some((peer, track)) = &slot.source {
                    self.queue.push_back(Output::Endpoint(
                        vec![endpoint.clone()],
                        ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotSet(index as u8, peer.clone(), track.clone())),
                    ));
                }
            }
        }
        self.endpoints.insert(endpoint, EndpointSlot { peer: peer.hash_code(), tracks });
    }

    /// We we receive audio pkt, we put it into a mixer, it the audio source is selected it will be forwarded to all endpoints except the origin peer.
    /// In case output don't have source info and audio pkt has source info, we set it and fire event in to all endpoints
    pub fn on_channel_data(&mut self, now: Instant, from: NodeId, pkt: &[u8]) {
        if self.endpoints.is_empty() {
            return;
        }
        let audio = return_if_none!(AudioMixerPkt::deserialize(pkt));
        if let Some((slot, just_set)) = self.mixer.on_pkt(now, (from, audio.slot), audio.audio_level) {
            // When a source is selected, we just reset the selected slot,
            // then wait for next audio pkt which carry source info
            if just_set {
                self.outputs[slot] = Some(OutputSlot { source: None });
            }

            // If selected slot dont have source info and audio pkt has it,
            // we will save it and fire event to all endpoints
            if audio.source.is_some() && self.outputs[slot].as_ref().expect("Output slot not found").source.is_none() {
                let (peer, track) = audio.source.clone().expect("Audio source not set");
                self.outputs[slot].as_mut().expect("Output slot not found").source = Some((peer.clone(), track.clone()));
                for (endpoint, _) in self.endpoints.iter() {
                    self.queue.push_back(Output::Endpoint(
                        vec![endpoint.clone()],
                        ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotSet(slot as u8, peer.clone(), track.clone())),
                    ));
                }
            }

            for (endpoint, endpoint_slot) in self.endpoints.iter() {
                if endpoint_slot.peer != audio.peer {
                    let track_id = endpoint_slot.tracks[slot];
                    if just_set {
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
                                    marker: false,
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

    pub fn on_endpoint_leave(&mut self, _now: Instant, endpoint: Endpoint) {
        assert!(self.endpoints.contains_key(&endpoint));
        log::info!("[ClusterAudioMixerSubscriber {OUTPUTS}] endpoint {:?} leave", endpoint);
        self.endpoints.swap_remove(&endpoint);
        if self.endpoints.is_empty() {
            log::info!("[ClusterAudioMixerSubscriber {OUTPUTS}] last endpoint leave in Auto mode => unsubscribe channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::UnsubAuto)));
            self.queue.push_back(Output::OnResourceEmpty);
        }
    }
}

impl<Endpoint, const OUTPUTS: usize> TaskSwitcherChild<Output<Endpoint>> for AudioMixerSubscriber<Endpoint, OUTPUTS> {
    type Time = ();
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint, const OUTPUTS: usize> Drop for AudioMixerSubscriber<Endpoint, OUTPUTS> {
    fn drop(&mut self) {
        log::info!("[ClusterAudioMixerSubscriber {OUTPUTS}] Drop {}", self.channel_id);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
        assert_eq!(self.endpoints.len(), 0, "Endpoints not empty on drop");
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use atm0s_sdn::features::pubsub;
    use media_server_protocol::{
        endpoint::{AudioMixerPkt, PeerId, TrackName},
        media::{MediaMeta, MediaPacket},
    };
    use sans_io_runtime::TaskSwitcherChild;

    use crate::cluster::{ClusterAudioMixerEvent, ClusterEndpointEvent, ClusterLocalTrackEvent};

    use super::{super::Output, AudioMixerSubscriber};

    fn ms(m: u64) -> Duration {
        Duration::from_millis(m)
    }

    #[test]
    fn sub_unsub() {
        let t0 = Instant::now();
        let channel = 0.into();
        let endpoint1 = 0;
        let peer1: PeerId = "peer1".into();
        let track1: TrackName = "audio".into();
        let endpoint2 = 1;
        let mut subscriber = AudioMixerSubscriber::<u8, 3>::new(channel);

        //first endpoint should fire Sub
        subscriber.on_endpoint_join(t0, endpoint1, peer1.clone(), vec![0.into(), 1.into(), 2.into()]);
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel, pubsub::ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(()), None);

        //next endpoint should not fire Sub
        subscriber.on_endpoint_join(t0, endpoint2, "peer2".into(), vec![0.into(), 1.into(), 2.into()]);
        assert_eq!(subscriber.pop_output(()), None);

        //incoming media should rely on audio mixer to forward to endpoints
        let node_id = 1;
        let pkt = MediaPacket {
            ts: 0,
            seq: 1,
            marker: false,
            nackable: false,
            layers: None,
            meta: MediaMeta::Opus { audio_level: Some(-60) },
            data: vec![1, 2, 3, 4, 5, 6],
        };
        let mixer_pkt = AudioMixerPkt {
            slot: 0,
            peer: peer1.hash_code(),
            track: 0.into(),
            audio_level: Some(-60),
            source: Some((peer1.clone(), track1.clone())),
            ts: 0,
            seq: 1,
            opus_payload: vec![1, 2, 3, 4, 5, 6],
        };
        let track_uuid = (mixer_pkt.peer.0 << 16) | (mixer_pkt.track.0 as u64);
        subscriber.on_channel_data(t0 + ms(100), node_id, &mixer_pkt.serialize());

        //sot 0 is set => fire AudioMixer::Set event
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Endpoint(
                vec![endpoint1],
                ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotSet(0, peer1.clone(), track1.clone()))
            ))
        );
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Endpoint(
                vec![endpoint2],
                ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotSet(0, peer1.clone(), track1.clone()))
            ))
        );

        //we only forward to peer2 because audio is not forward to same peer
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Endpoint(vec![endpoint2], ClusterEndpointEvent::LocalTrack(0.into(), ClusterLocalTrackEvent::SourceChanged)))
        );
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Endpoint(
                vec![endpoint2],
                ClusterEndpointEvent::LocalTrack(0.into(), ClusterLocalTrackEvent::Media(track_uuid, pkt))
            ))
        );

        //after tick timeout should fire unset
        subscriber.on_tick(t0 + ms(100 + 2000));
        //sot 0 is unset => fire AudioMixer::UnSet event
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Endpoint(vec![endpoint1], ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotUnset(0))))
        );
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Endpoint(vec![endpoint2], ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotUnset(0))))
        );

        //only last endpoint leave should fire Unsub
        subscriber.on_endpoint_leave(t0 + ms(100 + 2000), endpoint1);
        assert_eq!(subscriber.pop_output(()), None);

        //now is last endpoint => should fire Unsub
        subscriber.on_endpoint_leave(t0 + ms(100 + 2000), endpoint2);
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel, pubsub::ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(()), Some(Output::OnResourceEmpty));
        assert_eq!(subscriber.pop_output(()), None);
    }
}
