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
use media_server_utils::Count;
use sans_io_runtime::{collections::DynamicDeque, Task, TaskSwitcherChild};

use crate::cluster::{id_generator, ClusterAudioMixerEvent, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRoomHash};

use super::Output;

#[derive(Debug)]
pub enum Input {
    Attach(Vec<TrackSource>),
    Detach(Vec<TrackSource>),
    Pubsub(ChannelId, NodeId, Vec<u8>),
    LeaveRoom,
}

pub struct ManualMixer<Endpoint> {
    _c: Count<Self>,
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
            _c: Default::default(),
            endpoint,
            room,
            mixer: audio_mixer::AudioMixer::new(outputs.len()),
            outputs,
            sources: HashMap::new(),
            queue: Default::default(),
        }
    }

    fn attach(&mut self, _now: Instant, source: TrackSource) {
        let channel_id = id_generator::gen_track_channel_id(self.room, &source.peer, &source.track);
        if let std::collections::hash_map::Entry::Vacant(e) = self.sources.entry(channel_id) {
            log::info!("[ClusterManualMixer] add source {:?} => sub {channel_id}", source);
            e.insert(source);
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
        let channel_id = id_generator::gen_track_channel_id(self.room, &source.peer, &source.track);
        if self.sources.remove(&channel_id).is_some() {
            log::info!("[ClusterManualMixer] remove source {:?} => unsub {channel_id}", source);
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
            Input::LeaveRoom => {
                // We need manual release sources because it is from client request,
                // we cannot ensure client will release it before it disconnect.
                let sources = std::mem::take(&mut self.sources);
                for (channel_id, source) in sources {
                    log::info!("[ClusterManualMixer] remove source {:?} on queue => unsub {channel_id}", source);
                    self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::UnsubAuto)));
                }
            }
        }
    }

    fn on_shutdown(&mut self, _now: Instant) {
        // this is depend on endpoint, so we cannot shutdown until endpoint is empty
    }
}

impl<Endpoint> TaskSwitcherChild<Output<Endpoint>> for ManualMixer<Endpoint> {
    type Time = ();

    fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.sources.is_empty() && self.outputs.is_empty()
    }

    fn empty_event(&self) -> Output<Endpoint> {
        Output::OnResourceEmpty
    }

    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint> Drop for ManualMixer<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterManualMixer] Drop {}", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
        assert_eq!(self.sources.len(), 0, "Sources not empty on drop");
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use atm0s_sdn::features::pubsub;
    use media_server_protocol::{
        endpoint::TrackSource,
        media::{MediaMeta, MediaPacket},
    };
    use sans_io_runtime::{Task, TaskSwitcherChild};

    use crate::cluster::{id_generator, ClusterAudioMixerEvent, ClusterEndpointEvent, ClusterLocalTrackEvent};

    use super::{super::Output, Input, ManualMixer};

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn attach_detach() {
        let t0 = Instant::now();
        let room = 0.into();
        let endpoint = 1;
        let track = 0.into();
        let mut manual = ManualMixer::<u8>::new(room, endpoint, vec![track]);
        let source = TrackSource {
            peer: "peer1".into(),
            track: "audio".into(),
        };
        let channel_id = id_generator::gen_track_channel_id(room, &source.peer, &source.track);

        manual.attach(t0, source.clone());
        assert_eq!(manual.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::SubAuto))));
        assert_eq!(manual.pop_output(()), None);

        let pkt = MediaPacket {
            ts: 0,
            seq: 0,
            marker: false,
            nackable: false,
            layers: None,
            meta: MediaMeta::Opus { audio_level: Some(-60) },
            data: vec![1, 2, 3, 4, 5, 6],
        };
        manual.on_event(t0, Input::Pubsub(channel_id, 0, pkt.serialize()));
        assert_eq!(
            manual.pop_output(()),
            Some(Output::Endpoint(vec![endpoint], ClusterEndpointEvent::LocalTrack(track, ClusterLocalTrackEvent::SourceChanged)))
        );
        assert_eq!(
            manual.pop_output(()),
            Some(Output::Endpoint(
                vec![1],
                ClusterEndpointEvent::AudioMixer(ClusterAudioMixerEvent::SlotSet(0, source.peer.clone(), source.track.clone()))
            )),
        );
        assert_eq!(
            manual.pop_output(()),
            Some(Output::Endpoint(
                vec![endpoint],
                ClusterEndpointEvent::LocalTrack(track, ClusterLocalTrackEvent::Media(channel_id.0, pkt))
            )),
        );

        manual.detach(t0 + ms(100), source.clone());
        assert_eq!(manual.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::UnsubAuto))));
        assert_eq!(manual.pop_output(()), None);
    }

    #[test]
    fn leave_room() {
        let t0 = Instant::now();
        let room = 0.into();
        let endpoint = 1;
        let track = 0.into();
        let mut manual = ManualMixer::<u8>::new(room, endpoint, vec![track]);
        let source = TrackSource {
            peer: "peer1".into(),
            track: "audio".into(),
        };
        let channel_id = id_generator::gen_track_channel_id(room, &source.peer, &source.track);

        manual.attach(t0, source.clone());
        assert_eq!(manual.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::SubAuto))));
        assert_eq!(manual.pop_output(()), None);

        manual.on_event(t0, Input::LeaveRoom);
        assert_eq!(manual.pop_output(()), Some(Output::Pubsub(pubsub::Control(channel_id, pubsub::ChannelControl::UnsubAuto))));
        assert_eq!(manual.pop_output(()), None);
        assert_eq!(manual.is_empty(), true);
    }
}
