//!
//! This is manual mode of audio mixer.
//! In this mode, each peer has separated mixer logic and subscribe to all audio sources
//! to determine which source is sent to client.
//!

use std::{collections::HashMap, time::Instant};

use atm0s_sdn::{
    features::pubsub::{self, ChannelId},
    NodeId,
};
use media_server_protocol::{
    endpoint::TrackSource,
    media::{MediaMeta, MediaPacket},
    transport::LocalTrackId,
};
use sans_io_runtime::{collections::DynamicDeque, Task, TaskSwitcherChild};

use crate::cluster::{id_generator, ClusterAudioMixerEvent, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRoomHash};

use super::Output;

#[derive(Debug)]
pub enum Input {
    Attach(Vec<TrackSource>),
    Detach(Vec<TrackSource>),
    Pubsub(ChannelId, NodeId, Vec<u8>),
    Kill,
}

pub struct ManualMixer<Endpoint> {
    endpoint: Endpoint,
    room: ClusterRoomHash,
    outputs: Vec<LocalTrackId>,
    sources: HashMap<ChannelId, TrackSource>,
    queue: DynamicDeque<Output<Endpoint>, 4>,
    mixer: audio_mixer::AudioMixer<ChannelId>,
}

impl<Endpoint: Clone> ManualMixer<Endpoint> {
    pub fn new(room: ClusterRoomHash, endpoint: Endpoint, outputs: Vec<LocalTrackId>) -> Self {
        Self {
            endpoint,
            room,
            mixer: audio_mixer::AudioMixer::new(outputs.len()),
            outputs,
            sources: HashMap::new(),
            queue: Default::default(),
        }
    }

    fn attach(&mut self, _now: Instant, source: TrackSource) {
        let channel_id = id_generator::gen_channel_id(self.room, &source.peer, &source.track);
        if !self.sources.contains_key(&channel_id) {
            log::info!("[ManualMixer] add source {:?} => sub {channel_id}", source);
            self.sources.insert(channel_id, source);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::SubAuto)));
        }
    }

    fn on_source_pkt(&mut self, now: Instant, channel: ChannelId, _from: NodeId, pkt: MediaPacket) {
        if let MediaMeta::Opus { audio_level } = pkt.meta {
            if let Some((slot, just_set)) = self.mixer.on_pkt(now, channel, audio_level) {
                let track_id = self.outputs[slot];
                if just_set {
                    let source_info = self.sources.get(&channel).expect("Missing source info for channel");
                    self.queue.push_back(Output::Endpoint(
                        vec![self.endpoint.clone()],
                        ClusterEndpointEvent::LocalTrack(track_id, ClusterLocalTrackEvent::SourceChanged),
                    ));
                    self.queue.push_back(Output::Endpoint(
                        vec![self.endpoint.clone()],
                        ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotSet(slot as u8, source_info.peer.clone(), source_info.track.clone())),
                    ));
                }

                self.queue.push_back(Output::Endpoint(
                    vec![self.endpoint.clone()],
                    ClusterEndpointEvent::LocalTrack(track_id, ClusterLocalTrackEvent::Media(channel.0, pkt)),
                ))
            }
        }
    }

    fn detach(&mut self, _now: Instant, source: TrackSource) {
        let channel_id = id_generator::gen_channel_id(self.room, &source.peer, &source.track);
        if let Some(_) = self.sources.remove(&channel_id) {
            log::info!("[ManualMixer] remove source {:?} => unsub {channel_id}", source);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::UnsubAuto)));
        }
    }
}

impl<Endpoint: Clone> Task<Input, Output<Endpoint>> for ManualMixer<Endpoint> {
    fn on_tick(&mut self, now: Instant) {
        if let Some(removed) = self.mixer.on_tick(now) {
            for slot in removed {
                self.queue.push_back(Output::Endpoint(
                    vec![self.endpoint.clone()],
                    ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotUnset(slot as u8)),
                ));
            }
        }
    }

    fn on_event(&mut self, now: Instant, input: Input) {
        match input {
            Input::Attach(sources) => {
                for source in sources {
                    self.attach(now, source);
                }
            }
            Input::Detach(sources) => {
                for source in sources {
                    self.detach(now, source);
                }
            }
            Input::Pubsub(channel, from, data) => {
                if let Some(pkt) = MediaPacket::deserialize(&data) {
                    self.on_source_pkt(now, channel, from, pkt);
                }
            }
            Input::Kill => {
                let sources = std::mem::replace(&mut self.sources, Default::default());
                for (channel_id, source) in sources {
                    log::info!("[ManualMixer] remove source {:?} on queue => unsub {channel_id}", source);
                    self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::UnsubAuto)));
                }
            }
        }
    }

    fn on_shutdown(&mut self, _now: Instant) {}
}

impl<Endpoint> TaskSwitcherChild<Output<Endpoint>> for ManualMixer<Endpoint> {
    type Time = ();

    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint> Drop for ManualMixer<Endpoint> {
    fn drop(&mut self) {
        log::info!("Drop ManualMixer {}", self.room);
        assert_eq!(self.queue.len(), 0);
        assert_eq!(self.sources.len(), 0)
    }
}
