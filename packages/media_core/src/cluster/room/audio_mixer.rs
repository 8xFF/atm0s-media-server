//!
//! Audio mixer in room level is split to 2 part:
//! - Publisher: detect top 3 audio and publish to /room_id/audio_mixer channel
//! - Subscriber: subscribe to /room_id/audio_mixer to get all of top-3 audios from other servers
//!                 calculate top-3 audio for each local endpoint
//!

use std::{collections::HashMap, fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::{
    features::pubsub::{self, ChannelId},
    TimeTicker,
};
use media_server_protocol::{
    endpoint::{AudioMixerConfig, PeerId, TrackName},
    media::MediaPacket,
};
use sans_io_runtime::{TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::{cluster::ClusterEndpointEvent, transport::RemoteTrackId};

use publisher::AudioMixerPublisher;
use subscriber::AudioMixerSubscriber;

mod publisher;
mod subscriber;

#[derive(num_enum::IntoPrimitive, num_enum::TryFromPrimitive)]
#[repr(usize)]
pub enum TaskType {
    Publisher = 0,
    Subscriber = 1,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output<Endpoint> {
    Endpoint(Vec<Endpoint>, ClusterEndpointEvent),
    Pubsub(pubsub::Control),
}

pub struct AudioMixer<Endpoint> {
    auto_mode: HashMap<Endpoint, PeerId>,
    publisher: TaskSwitcherBranch<AudioMixerPublisher<Endpoint>, Output<Endpoint>>,
    subscriber: TaskSwitcherBranch<AudioMixerSubscriber<Endpoint>, Output<Endpoint>>,
    switcher: TaskSwitcher,
    last_tick: u64,
}

impl<Endpoint: Debug + Clone + Hash + Eq> AudioMixer<Endpoint> {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            auto_mode: HashMap::new(),
            publisher: TaskSwitcherBranch::new(AudioMixerPublisher::new(channel_id), TaskType::Publisher),
            subscriber: TaskSwitcherBranch::new(AudioMixerSubscriber::new(channel_id), TaskType::Subscriber),
            switcher: TaskSwitcher::new(2),
            last_tick: 0,
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        if now_ms >= self.last_tick + 1000 {
            self.last_tick = now_ms;
            self.publisher.input(&mut self.switcher).on_tick(now_ms);
            self.subscriber.input(&mut self.switcher).on_tick(now_ms);
        }
    }

    pub fn on_join(&mut self, endpoint: Endpoint, peer: PeerId, cfg: Option<AudioMixerConfig>) {
        if let Some(cfg) = cfg {
            match cfg.mode {
                media_server_protocol::endpoint::AudioMixerMode::Auto => {
                    self.auto_mode.insert(endpoint.clone(), peer.clone());
                    self.subscriber.input(&mut self.switcher).on_endpoint_join(endpoint, peer, cfg.outputs);
                }
                media_server_protocol::endpoint::AudioMixerMode::Manual => todo!(),
            }
        }
    }

    pub fn on_leave(&mut self, endpoint: Endpoint) {
        if let Some(peer) = self.auto_mode.remove(&endpoint) {
            self.subscriber.input(&mut self.switcher).on_endpoint_leave(endpoint);
        }
    }

    pub fn on_track_publish(&mut self, now: u64, endpoint: Endpoint, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        if self.auto_mode.contains_key(&endpoint) {
            self.publisher.input(&mut self.switcher).on_track_publish(now, endpoint, track, peer, name);
        }
    }

    pub fn on_track_data(&mut self, now: u64, endpoint: Endpoint, track: RemoteTrackId, media: &MediaPacket) {
        if self.auto_mode.contains_key(&endpoint) {
            self.publisher.input(&mut self.switcher).on_track_data(now, endpoint, track, media);
        }
    }

    pub fn on_track_unpublish(&mut self, now: u64, endpoint: Endpoint, track: RemoteTrackId) {
        if self.auto_mode.contains_key(&endpoint) {
            self.publisher.input(&mut self.switcher).on_track_unpublish(now, endpoint, track);
        }
    }

    pub fn on_pubsub_event(&mut self, now: u64, event: pubsub::Event) {
        match event.1 {
            pubsub::ChannelEvent::RouteChanged(next) => {}
            pubsub::ChannelEvent::SourceData(from, data) => {
                self.subscriber.input(&mut self.switcher).on_channel_data(now, from, data);
            }
            pubsub::ChannelEvent::FeedbackData(fb) => {}
        }
    }
}

impl<Endpoint> TaskSwitcherChild<Output<Endpoint>> for AudioMixer<Endpoint> {
    type Time = Instant;

    fn pop_output(&mut self, now: Self::Time) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Publisher => {
                    if let Some(out) = self.publisher.pop_output(now, &mut self.switcher) {
                        return Some(out);
                    }
                }
                TaskType::Subscriber => {
                    if let Some(out) = self.subscriber.pop_output(now, &mut self.switcher) {
                        return Some(out);
                    }
                }
            }
        }
    }
}
