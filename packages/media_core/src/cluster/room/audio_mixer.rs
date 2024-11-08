//!
//! Audio mixer in room level is split to 2 part:
//! - Publisher: detect top 3 audio and publish to /room_id/audio_mixer channel
//! - Subscriber: subscribe to /room_id/audio_mixer to get all of top-3 audios from other servers
//!                 calculate top-3 audio for each local endpoint
//!

//TODO refactor multiple subscriber mode to array instead of manual implement with subscriber1, subscriber2, subscriber3

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
use sans_io_runtime::{TaskGroup, TaskGroupOutput, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

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
    Subscriber1,
    Subscriber2,
    Subscriber3,
    Manuals,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output<Endpoint> {
    Endpoint(Vec<Endpoint>, ClusterEndpointEvent),
    Pubsub(pubsub::Control),
    OnResourceEmpty,
}

type AudioMixerManuals<T> = TaskSwitcherBranch<TaskGroup<manual::Input, Output<T>, ManualMixer<T>, 4>, (usize, Output<T>)>;

pub struct AudioMixer<Endpoint: Clone> {
    room: ClusterRoomHash,
    mix_channel_id: ChannelId,
    //store number of outputs
    auto_mode: HashMap<Endpoint, usize>,
    manual_mode: HashMap<Endpoint, usize>,
    manual_channels: HashMap<ChannelId, Vec<usize>>,
    publisher: TaskSwitcherBranch<AudioMixerPublisher<Endpoint>, Output<Endpoint>>,
    subscriber1: TaskSwitcherBranch<AudioMixerSubscriber<Endpoint, 1>, Output<Endpoint>>,
    subscriber2: TaskSwitcherBranch<AudioMixerSubscriber<Endpoint, 2>, Output<Endpoint>>,
    subscriber3: TaskSwitcherBranch<AudioMixerSubscriber<Endpoint, 3>, Output<Endpoint>>,
    manuals: AudioMixerManuals<Endpoint>,
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
            subscriber1: TaskSwitcherBranch::new(AudioMixerSubscriber::new(mix_channel_id), TaskType::Subscriber1),
            subscriber2: TaskSwitcherBranch::new(AudioMixerSubscriber::new(mix_channel_id), TaskType::Subscriber2),
            subscriber3: TaskSwitcherBranch::new(AudioMixerSubscriber::new(mix_channel_id), TaskType::Subscriber3),
            manuals: TaskSwitcherBranch::new(Default::default(), TaskType::Manuals),
            switcher: TaskSwitcher::new(5),
            last_tick: Instant::now(),
        }
    }

    pub fn on_tick(&mut self, now: Instant) {
        if now >= self.last_tick + TICK_INTERVAL {
            self.last_tick = now;
            self.publisher.input(&mut self.switcher).on_tick(now);
            self.subscriber1.input(&mut self.switcher).on_tick(now);
            self.subscriber2.input(&mut self.switcher).on_tick(now);
            self.subscriber3.input(&mut self.switcher).on_tick(now);
            self.manuals.input(&mut self.switcher).on_tick(now);
        }
    }

    pub fn on_join(&mut self, now: Instant, endpoint: Endpoint, peer: PeerId, cfg: Option<AudioMixerConfig>) {
        if let Some(cfg) = cfg {
            match cfg.mode {
                media_server_protocol::endpoint::AudioMixerMode::Auto => {
                    self.auto_mode.insert(endpoint.clone(), cfg.outputs.len());
                    match cfg.outputs.len() {
                        1 => self.subscriber1.input(&mut self.switcher).on_endpoint_join(now, endpoint, peer, cfg.outputs),
                        2 => self.subscriber2.input(&mut self.switcher).on_endpoint_join(now, endpoint, peer, cfg.outputs),
                        3 => self.subscriber3.input(&mut self.switcher).on_endpoint_join(now, endpoint, peer, cfg.outputs),
                        _ => {
                            log::warn!("[ClusterRoomAudioMixer] unsupported mixer with {} outputs", cfg.outputs.len());
                        }
                    }
                }
                media_server_protocol::endpoint::AudioMixerMode::Manual => {
                    log::info!("[ClusterRoomAudioMixer] add manual mode for {:?} {peer}", endpoint);
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
        log::info!("[ClusterRoomAudioMixer] on endpoint {:?} input {:?}", endpoint, control);
        let index = *self.manual_mode.get(&endpoint).expect("Manual mixer not found for control");
        let input = match control {
            ClusterAudioMixerControl::Attach(sources) => manual::Input::Attach(sources),
            ClusterAudioMixerControl::Detach(sources) => manual::Input::Detach(sources),
        };
        self.manuals.input(&mut self.switcher).on_event(now, index, input);
    }

    pub fn on_leave(&mut self, now: Instant, endpoint: Endpoint) {
        if let Some(outputs) = self.auto_mode.remove(&endpoint) {
            match outputs {
                1 => self.subscriber1.input(&mut self.switcher).on_endpoint_leave(now, endpoint),
                2 => self.subscriber2.input(&mut self.switcher).on_endpoint_leave(now, endpoint),
                3 => self.subscriber3.input(&mut self.switcher).on_endpoint_leave(now, endpoint),
                _ => {
                    log::warn!("[ClusterRoomAudioMixer] unsupported mixer with {} outputs", outputs);
                }
            }
        } else if let Some(index) = self.manual_mode.remove(&endpoint) {
            log::info!("[ClusterRoomAudioMixer] endpoint {:?} leave from manual mode", endpoint);
            self.manual_mode.remove(&endpoint);
            self.manuals.input(&mut self.switcher).on_event(now, index, manual::Input::LeaveRoom);
        }
    }

    pub fn on_track_publish(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        self.publisher.input(&mut self.switcher).on_track_publish(now, endpoint, track, peer, name);
    }

    pub fn on_track_data(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId, media: &MediaPacket) {
        self.publisher.input(&mut self.switcher).on_track_data(now, endpoint, track, media);
    }

    pub fn on_track_unpublish(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId) {
        self.publisher.input(&mut self.switcher).on_track_unpublish(now, endpoint, track);
    }

    pub fn on_pubsub_event(&mut self, now: Instant, event: pubsub::Event) {
        match event.1 {
            pubsub::ChannelEvent::RouteChanged(_next) => {}
            pubsub::ChannelEvent::SourceData(from, data) => {
                if event.0 == self.mix_channel_id {
                    self.subscriber1.input(&mut self.switcher).on_channel_data(now, from, &data);
                    self.subscriber2.input(&mut self.switcher).on_channel_data(now, from, &data);
                    self.subscriber3.input(&mut self.switcher).on_channel_data(now, from, &data);
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

    fn is_empty(&self) -> bool {
        self.manual_channels.is_empty()
            && self.manual_mode.is_empty()
            && self.publisher.is_empty()
            && self.subscriber1.is_empty()
            && self.subscriber2.is_empty()
            && self.subscriber3.is_empty()
            && self.manuals.is_empty()
    }

    fn empty_event(&self) -> Output<Endpoint> {
        Output::OnResourceEmpty
    }

    ///
    /// We need to wait all publisher, subscriber, and manuals ready to remove
    ///
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Publisher => {
                    if let Some(out) = self.publisher.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            // we dont need to forward OnResourceEmpty to parent
                        } else {
                            return Some(out);
                        }
                    }
                }
                TaskType::Subscriber1 => {
                    if let Some(out) = self.subscriber1.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            // we dont need to forward OnResourceEmpty to parent
                        } else {
                            return Some(out);
                        }
                    }
                }
                TaskType::Subscriber2 => {
                    if let Some(out) = self.subscriber2.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            // we dont need to forward OnResourceEmpty to parent
                        } else {
                            return Some(out);
                        }
                    }
                }
                TaskType::Subscriber3 => {
                    if let Some(out) = self.subscriber3.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            // we dont need to forward OnResourceEmpty to parent
                        } else {
                            return Some(out);
                        }
                    }
                }
                TaskType::Manuals => {
                    let (index, out) = match self.manuals.input(&mut self.switcher).pop_output(()) {
                        Some(TaskGroupOutput::TaskOutput(index, out)) => (index, out),
                        Some(TaskGroupOutput::OnResourceEmpty) => {
                            // we dont need to forward OnResourceEmpty to parent
                            continue;
                        }
                        None => {
                            continue;
                        }
                    };

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

impl<Endpoint: Clone> Drop for AudioMixer<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomAudioMixer] Drop {}", self.room);
        assert_eq!(self.manual_channels.len(), 0, "Manual channels not empty on drop");
        assert_eq!(self.manual_mode.len(), 0, "Manual modes not empty on drop");
        assert!(self.manuals.is_empty(), "AudioMixerManuals not empty on drop");
        assert!(self.publisher.is_empty(), "AudioMixerPublisher not empty on drop");
        assert!(self.subscriber1.is_empty(), "AudioMixerSubscriber1 not empty on drop");
        assert!(self.subscriber2.is_empty(), "AudioMixerSubscriber2 not empty on drop");
        assert!(self.subscriber3.is_empty(), "AudioMixerSubscriber3 not empty on drop");
    }
}
