use std::{collections::HashMap, fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::features::pubsub::{self, ChannelId};
use media_server_protocol::{
    endpoint::{AudioMixerPkt, PeerHashCode, PeerId, TrackName},
    media::{MediaMeta, MediaPacket},
};
use sans_io_runtime::{collections::DynamicDeque, return_if_none, TaskSwitcherChild};

use crate::transport::RemoteTrackId;

use super::Output;

pub struct AudioMixerPublisher<Endpoint> {
    channel_id: pubsub::ChannelId,
    tracks: HashMap<(Endpoint, RemoteTrackId), PeerHashCode>,
    mixer: audio_mixer::AudioMixer<(Endpoint, RemoteTrackId)>,
    queue: DynamicDeque<Output<Endpoint>, 4>,
}

impl<Endpoint: Debug + Clone + Eq + Hash> AudioMixerPublisher<Endpoint> {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            tracks: Default::default(),
            channel_id,
            mixer: audio_mixer::AudioMixer::new(3),
            queue: DynamicDeque::default(),
        }
    }

    pub fn on_tick(&mut self, now: u64) {
        if let Some(removed_slots) = self.mixer.on_tick(now) {}
    }

    pub fn on_track_publish(&mut self, now: u64, endpoint: Endpoint, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        log::debug!("on track publish {peer}/{name}/{track}");
        let key = (endpoint, track);
        assert!(!self.tracks.contains_key(&key));
        if self.tracks.is_empty() {
            log::info!("[AudioMixerPublisher] first track join as Auto mode => publish channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::PubStart)));
        }
        self.tracks.insert(key.clone(), peer.hash_code());
    }

    pub fn on_track_data(&mut self, now: u64, endpoint: Endpoint, track: RemoteTrackId, media: &MediaPacket) {
        let key = (endpoint, track);
        if let MediaMeta::Opus { audio_level } = &media.meta {
            if let Some((slot, _just_pinned)) = self.mixer.on_pkt(now, key.clone(), *audio_level) {
                let info = return_if_none!(self.tracks.get(&key));
                let pkt = AudioMixerPkt {
                    slot: slot as u8,
                    peer: *info,
                    track,
                    audio_level: *audio_level,
                    source: None,
                    ts: media.ts,
                    seq: media.seq,
                    opus_payload: media.data.clone(),
                };
                self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::PubData(pkt.serialize()))))
            }
        }
    }

    pub fn on_track_unpublish(&mut self, now: u64, endpoint: Endpoint, track: RemoteTrackId) {
        log::debug!("on track unpublish {track}");
        let key = (endpoint, track);
        assert!(self.tracks.contains_key(&key));
        self.tracks.remove(&key);
        if self.tracks.is_empty() {
            log::info!("[AudioMixerPublisher] last track leave ind Auto mode => unpublish channel {}", self.channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(self.channel_id, pubsub::ChannelControl::PubStop)));
        }
    }
}

impl<Endpoint> TaskSwitcherChild<Output<Endpoint>> for AudioMixerPublisher<Endpoint> {
    type Time = Instant;
    fn pop_output(&mut self, now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}
