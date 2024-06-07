use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    time::{Duration, Instant},
};

use atm0s_sdn::features::pubsub::{self, ChannelId};
use media_server_protocol::{
    endpoint::{AudioMixerPkt, PeerHashCode, PeerId, TrackName},
    media::{MediaMeta, MediaPacket},
};
use sans_io_runtime::{collections::DynamicDeque, TaskSwitcherChild};

use crate::transport::RemoteTrackId;

use super::Output;

const FIRE_SOURCE_INTERVAL: Duration = Duration::from_millis(500);

struct TrackSlot {
    peer: PeerId,
    name: TrackName,
    peer_hash: PeerHashCode,
}

struct OutputSlot {
    last_fired_source: Instant,
}

pub struct AudioMixerPublisher<Endpoint> {
    channel_id: pubsub::ChannelId,
    tracks: HashMap<(Endpoint, RemoteTrackId), TrackSlot>,
    mixer: audio_mixer::AudioMixer<(Endpoint, RemoteTrackId)>,
    slots: [Option<OutputSlot>; 3],
    queue: DynamicDeque<Output<Endpoint>, 4>,
}

impl<Endpoint: Debug + Clone + Eq + Hash> AudioMixerPublisher<Endpoint> {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            tracks: Default::default(),
            channel_id,
            mixer: audio_mixer::AudioMixer::new(3),
            slots: [None, None, None],
            queue: DynamicDeque::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn on_tick(&mut self, now: Instant) {
        if let Some(removed_slots) = self.mixer.on_tick(now) {
            for slot in removed_slots {
                self.slots[slot] = None;
            }
        }
    }

    pub fn on_track_publish(&mut self, _now: Instant, endpoint: Endpoint, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        log::debug!("on track publish {peer}/{name}/{track}");
        let key = (endpoint, track);
        assert!(!self.tracks.contains_key(&key));
        if self.tracks.is_empty() {
            log::info!("[ClusterAudioMixerPublisher] first track join as Auto mode => publish channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::PubStart)));
        }
        self.tracks.insert(
            key.clone(),
            TrackSlot {
                peer_hash: peer.hash_code(),
                peer,
                name,
            },
        );
    }

    pub fn on_track_data(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId, media: &MediaPacket) {
        let key = (endpoint, track);
        let info = self.tracks.get(&key).expect("Track not found");
        if let MediaMeta::Opus { audio_level } = &media.meta {
            if let Some((slot, just_set)) = self.mixer.on_pkt(now, key.clone(), *audio_level) {
                let mut source = None;
                if just_set {
                    self.slots[slot] = Some(OutputSlot { last_fired_source: now });
                    source = Some((info.peer.clone(), info.name.clone()));
                } else {
                    let slot_info = self.slots[slot].as_mut().expect("Output slot not found");
                    if slot_info.last_fired_source + FIRE_SOURCE_INTERVAL <= now {
                        slot_info.last_fired_source = now;
                        source = Some((info.peer.clone(), info.name.clone()));
                    }
                };
                let pkt = AudioMixerPkt {
                    slot: slot as u8,
                    peer: info.peer_hash,
                    track,
                    audio_level: *audio_level,
                    source,
                    ts: media.ts,
                    seq: media.seq,
                    opus_payload: media.data.clone(),
                };
                self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::PubData(pkt.serialize()))))
            }
        }
    }

    pub fn on_track_unpublish(&mut self, _now: Instant, endpoint: Endpoint, track: RemoteTrackId) {
        log::debug!("[ClusterAudioMixerPublisher] on track unpublish {track}");
        let key = (endpoint, track);
        assert!(self.tracks.contains_key(&key));
        self.tracks.remove(&key);
        if self.tracks.is_empty() {
            log::info!("[ClusterAudioMixerPublisher] last track leave ind Auto mode => unpublish channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::PubStop)));
            self.queue.push_back(Output::OnResourceEmpty);
        }
    }
}

impl<Endpoint> TaskSwitcherChild<Output<Endpoint>> for AudioMixerPublisher<Endpoint> {
    type Time = ();
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint> Drop for AudioMixerPublisher<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterAudioMixerPublisher] Drop {}", self.channel_id);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
        assert_eq!(self.tracks.len(), 0, "Tracks not empty on drop");
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use atm0s_sdn::features::pubsub;
    use media_server_protocol::{
        endpoint::{AudioMixerPkt, PeerId},
        media::{MediaMeta, MediaPacket},
    };
    use sans_io_runtime::TaskSwitcherChild;

    use super::{super::Output, AudioMixerPublisher};

    fn ms(m: u64) -> Duration {
        Duration::from_millis(m)
    }

    #[test]
    fn track_publish_unpublish() {
        let channel = 0.into();
        let peer1: PeerId = "peer1".into();
        let peer2: PeerId = "peer2".into();

        let mut publisher = AudioMixerPublisher::<u8>::new(channel);

        let t0 = Instant::now();

        publisher.on_track_publish(t0, 1, 0.into(), peer1.clone(), "audio".into());
        assert_eq!(publisher.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel, pubsub::ChannelControl::PubStart))));
        assert_eq!(publisher.pop_output(()), None);

        //same endpoint publish more track should not start channel
        publisher.on_track_publish(t0, 1, 1.into(), peer1.clone(), "audio2".into());
        assert_eq!(publisher.pop_output(()), None);

        //other endpoint publish more track should not start channel
        publisher.on_track_publish(t0, 2, 0.into(), peer2, "audio".into());
        assert_eq!(publisher.pop_output(()), None);

        //when have track data, depend on audio mixer output, it will push to pubsub. in this case we have 3 output then all data is published
        let pkt = MediaPacket {
            ts: 0,
            seq: 1,
            marker: false,
            nackable: false,
            layers: None,
            meta: MediaMeta::Opus { audio_level: Some(-60) },
            data: vec![1, 2, 3, 4, 5, 6],
        };
        publisher.on_track_data(t0, 1, 0.into(), &pkt);
        let expected_pub = AudioMixerPkt {
            slot: 0,
            peer: peer1.hash_code(),
            track: 0.into(),
            audio_level: Some(-60),
            source: Some((peer1.clone(), "audio".into())),
            ts: 0,
            seq: 1,
            opus_payload: vec![1, 2, 3, 4, 5, 6],
        };
        assert_eq!(
            publisher.pop_output(()),
            Some(Output::Pubsub(pubsub::Control(channel, pubsub::ChannelControl::PubData(expected_pub.serialize()))))
        );
        assert_eq!(publisher.pop_output(()), None);

        //only last track leaved will generate PubStop
        publisher.on_track_unpublish(t0 + ms(100), 1, 0.into());
        assert_eq!(publisher.pop_output(()), None);

        publisher.on_track_unpublish(t0 + ms(100), 1, 1.into());
        assert_eq!(publisher.pop_output(()), None);

        publisher.on_track_unpublish(t0 + ms(100), 2, 0.into());
        assert_eq!(publisher.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel, pubsub::ChannelControl::PubStop))));
        assert_eq!(publisher.pop_output(()), None);
    }

    #[test]
    #[should_panic(expected = "Track not found")]
    fn invalid_track_data_should_panic() {
        let t0 = Instant::now();
        let mut publisher = AudioMixerPublisher::<u8>::new(0.into());
        let pkt = MediaPacket {
            ts: 0,
            seq: 1,
            marker: false,
            nackable: false,
            layers: None,
            meta: MediaMeta::Opus { audio_level: Some(-60) },
            data: vec![1, 2, 3, 4, 5, 6],
        };
        publisher.on_track_data(t0, 1, 1.into(), &pkt);
    }
}
