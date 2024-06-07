//!
//! Audio mixer in room level is split to 2 part:
//! - Publisher: detect top 3 audio and publish to /room_id/audio_mixer channel
//! - Subscriber: subscribe to /room_id/audio_mixer to get all of top-3 audios from other servers
//!                 calculate top-3 audio for each local endpoint
//!

use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    time::{Duration, Instant},
};

use atm0s_sdn::features::pubsub::{self, ChannelId};
use manual::ManualMixer;
use media_server_protocol::{
    endpoint::{AudioMixerConfig, PeerId, TrackName},
    media::MediaPacket,
};
use sans_io_runtime::{TaskGroup, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::{
    cluster::{ClusterAudioMixerControl, ClusterEndpointEvent, ClusterRoomHash},
    transport::RemoteTrackId,
};

use publisher::AudioMixerPublisher;
use subscriber::AudioMixerSubscriber;

mod manual;
mod publisher;
mod subscriber;

const TICK_INTERVAL: Duration = Duration::from_millis(1000);

#[derive(num_enum::IntoPrimitive, num_enum::TryFromPrimitive)]
#[repr(usize)]
pub enum TaskType {
    Publisher,
    Subscriber,
    Manuals,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output<Endpoint> {
    Endpoint(Vec<Endpoint>, ClusterEndpointEvent),
    Pubsub(pubsub::Control),
    OnResourceEmpty,
}

pub struct AudioMixer<Endpoint: Clone> {
    room: ClusterRoomHash,
    mix_channel_id: ChannelId,
    auto_mode: HashMap<Endpoint, PeerId>,
    manual_mode: HashMap<Endpoint, usize>,
    manual_channels: HashMap<ChannelId, Vec<usize>>,
    publisher: TaskSwitcherBranch<AudioMixerPublisher<Endpoint>, Output<Endpoint>>,
    subscriber: TaskSwitcherBranch<AudioMixerSubscriber<Endpoint>, Output<Endpoint>>,
    manuals: TaskSwitcherBranch<TaskGroup<manual::Input, Output<Endpoint>, ManualMixer<Endpoint>, 4>, (usize, Output<Endpoint>)>,
    switcher: TaskSwitcher,
    last_tick: Instant,
}

impl<Endpoint: Debug + Clone + Hash + Eq> AudioMixer<Endpoint> {
    pub fn new(room: ClusterRoomHash, mix_channel_id: ChannelId) -> Self {
        Self {
            room,
            mix_channel_id,
            auto_mode: HashMap::new(),
            manual_mode: HashMap::new(),
            manual_channels: HashMap::new(),
            publisher: TaskSwitcherBranch::new(AudioMixerPublisher::new(mix_channel_id), TaskType::Publisher),
            subscriber: TaskSwitcherBranch::new(AudioMixerSubscriber::new(mix_channel_id), TaskType::Subscriber),
            manuals: TaskSwitcherBranch::new(Default::default(), TaskType::Manuals),
            switcher: TaskSwitcher::new(3),
            last_tick: Instant::now(),
        }
    }

    ///
    /// We need to wait all publisher, subscriber, and manuals ready to remove
    ///
    pub fn is_empty(&self) -> bool {
        self.publisher.is_empty() && self.subscriber.is_empty() && self.manuals.tasks() == 0
    }

    pub fn on_tick(&mut self, now: Instant) {
        if now >= self.last_tick + TICK_INTERVAL {
            self.last_tick = now;
            self.publisher.input(&mut self.switcher).on_tick(now);
            self.subscriber.input(&mut self.switcher).on_tick(now);
            self.manuals.input(&mut self.switcher).on_tick(now);
        }
    }

    pub fn on_join(&mut self, now: Instant, endpoint: Endpoint, peer: PeerId, cfg: Option<AudioMixerConfig>) {
        if let Some(cfg) = cfg {
            match cfg.mode {
                media_server_protocol::endpoint::AudioMixerMode::Auto => {
                    self.auto_mode.insert(endpoint.clone(), peer.clone());
                    self.subscriber.input(&mut self.switcher).on_endpoint_join(now, endpoint, peer, cfg.outputs);
                }
                media_server_protocol::endpoint::AudioMixerMode::Manual => {
                    log::info!("[ClusterAudioMixer] add manual mode for {:?} {peer}", endpoint);
                    let manual_mixer = ManualMixer::new(self.room, endpoint.clone(), cfg.outputs);
                    let new_index = self.manuals.input(&mut self.switcher).add_task(manual_mixer);
                    if let Some(_old_index) = self.manual_mode.insert(endpoint, new_index) {
                        panic!("Manual mixer for endpoint already exist");
                    }
                }
            }
        }
    }

    pub fn on_control(&mut self, now: Instant, endpoint: Endpoint, control: ClusterAudioMixerControl) {
        log::info!("[ClusterAudioMixer] on endpoint {:?} input {:?}", endpoint, control);
        let index = *self.manual_mode.get(&endpoint).expect("Manual mixer not found for control");
        let input = match control {
            ClusterAudioMixerControl::Attach(sources) => manual::Input::Attach(sources),
            ClusterAudioMixerControl::Detach(sources) => manual::Input::Detach(sources),
        };
        self.manuals.input(&mut self.switcher).on_event(now, index, input);
    }

    pub fn on_leave(&mut self, now: Instant, endpoint: Endpoint) {
        if let Some(_peer) = self.auto_mode.remove(&endpoint) {
            self.subscriber.input(&mut self.switcher).on_endpoint_leave(now, endpoint);
        } else if let Some(index) = self.manual_mode.remove(&endpoint) {
            log::info!("[ClusterAudioMixer] endpoint {:?} leave from manual mode", endpoint);
            self.manual_mode.remove(&endpoint);
            self.manuals.input(&mut self.switcher).on_event(now, index, manual::Input::LeaveRoom);
        }
    }

    pub fn on_track_publish(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        if self.auto_mode.contains_key(&endpoint) {
            self.publisher.input(&mut self.switcher).on_track_publish(now, endpoint, track, peer, name);
        }
    }

    pub fn on_track_data(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId, media: &MediaPacket) {
        if self.auto_mode.contains_key(&endpoint) {
            self.publisher.input(&mut self.switcher).on_track_data(now, endpoint, track, media);
        }
    }

    pub fn on_track_unpublish(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId) {
        if self.auto_mode.contains_key(&endpoint) {
            self.publisher.input(&mut self.switcher).on_track_unpublish(now, endpoint, track);
        }
    }

    pub fn on_pubsub_event(&mut self, now: Instant, event: pubsub::Event) {
        match event.1 {
            pubsub::ChannelEvent::RouteChanged(_next) => {}
            pubsub::ChannelEvent::SourceData(from, data) => {
                if event.0 == self.mix_channel_id {
                    self.subscriber.input(&mut self.switcher).on_channel_data(now, from, data);
                } else if let Some(tasks) = self.manual_channels.get(&event.0) {
                    for task_index in tasks {
                        self.manuals.input(&mut self.switcher).on_event(now, *task_index, manual::Input::Pubsub(event.0, from, data.clone()));
                    }
                }
            }
            pubsub::ChannelEvent::FeedbackData(_fb) => {}
        }
    }
}

impl<Endpoint: Debug + Clone + Hash + Eq> TaskSwitcherChild<Output<Endpoint>> for AudioMixer<Endpoint> {
    type Time = ();

    ///
    /// We need to wait all publisher, subscriber, and manuals ready to remove
    ///
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Publisher => {
                    if let Some(out) = self.publisher.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            if self.is_empty() {
                                return Some(Output::OnResourceEmpty);
                            }
                        } else {
                            return Some(out);
                        }
                    }
                }
                TaskType::Subscriber => {
                    if let Some(out) = self.subscriber.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            if self.is_empty() {
                                return Some(Output::OnResourceEmpty);
                            }
                        } else {
                            return Some(out);
                        }
                    }
                }
                TaskType::Manuals => {
                    if let Some((index, out)) = self.manuals.pop_output((), &mut self.switcher) {
                        match out {
                            Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::SubAuto)) => {
                                if let Some(slot) = self.manual_channels.get_mut(&channel_id) {
                                    slot.push(index);
                                } else {
                                    self.manual_channels.insert(channel_id, vec![index]);
                                    return Some(out);
                                }
                            }
                            Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::UnsubAuto)) => {
                                let slot = self.manual_channels.get_mut(&channel_id).expect("Manual channel map not found");
                                let (slot_index, _) = slot.iter().enumerate().find(|(_, task_i)| **task_i == index).expect("Subscribed task not found");
                                slot.swap_remove(slot_index);
                                if slot.is_empty() {
                                    self.manual_channels.remove(&channel_id);
                                    return Some(out);
                                }
                            }
                            Output::OnResourceEmpty => {
                                self.manuals.input(&mut self.switcher).remove_task(index);
                                if self.is_empty() {
                                    return Some(Output::OnResourceEmpty);
                                }
                            }
                            _ => return Some(out),
                        }
                    }
                }
            }
        }
    }
}

impl<Endpoint: Clone> Drop for AudioMixer<Endpoint> {
    fn drop(&mut self) {
        log::info!("Drop AudioMixer {}", self.room);
        assert_eq!(self.manual_channels.len(), 0, "Manual channels not empty on drop");
        assert_eq!(self.manual_mode.len(), 0, "Manual modes not empty on drop");
        assert_eq!(self.manuals.tasks(), 0, "Manuals not empty on drop");
    }
}
