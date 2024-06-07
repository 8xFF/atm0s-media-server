//!
//! This part is composer from some other small parts: Metadata, Channel Subscriber, Channel Publisher
//!
//! Main functions:
//!
//! - Send/Recv metadata related key-value
//! - Send/Recv media channel
//! - AudioMixer feature
//!

use std::{fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::{
    features::{dht_kv, FeaturesControl, FeaturesEvent},
    TimePivot,
};
use sans_io_runtime::{return_if_none, Task, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::transport::{LocalTrackId, RemoteTrackId};

use audio_mixer::AudioMixer;
use media_track::MediaTrack;
use metadata::RoomMetadata;

use super::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackControl, ClusterRemoteTrackControl, ClusterRoomHash};

mod audio_mixer;
mod media_track;
mod metadata;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum RoomFeature {
    MetaData,
    MediaTrack,
    AudioMixer,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct RoomUserData(pub(crate) ClusterRoomHash, pub(crate) RoomFeature);

pub enum Input<Endpoint> {
    Sdn(RoomUserData, FeaturesEvent),
    Endpoint(Endpoint, ClusterEndpointControl),
}

pub enum Output<Endpoint> {
    Sdn(RoomUserData, FeaturesControl),
    Endpoint(Vec<Endpoint>, ClusterEndpointEvent),
    Destroy(ClusterRoomHash),
}

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(usize)]
enum TaskType {
    Metadata,
    MediaTrack,
    AudioMixer,
}

pub struct ClusterRoom<Endpoint> {
    room: ClusterRoomHash,
    metadata: TaskSwitcherBranch<RoomMetadata<Endpoint>, metadata::Output<Endpoint>>,
    media_track: TaskSwitcherBranch<MediaTrack<Endpoint>, media_track::Output<Endpoint>>,
    audio_mixer: TaskSwitcherBranch<AudioMixer<Endpoint>, audio_mixer::Output<Endpoint>>,
    switcher: TaskSwitcher,
    time_pivot: TimePivot,
}

impl<Endpoint: Debug + Copy + Clone + Hash + Eq> Task<Input<Endpoint>, Output<Endpoint>> for ClusterRoom<Endpoint> {
    fn on_tick(&mut self, now: Instant) {
        let now_ms = self.time_pivot.timestamp_ms(now);
        self.audio_mixer.input(&mut self.switcher).on_tick(now_ms);
    }

    fn on_event(&mut self, now: Instant, input: Input<Endpoint>) {
        match input {
            Input::Endpoint(endpoint, control) => self.on_endpoint_control(now, endpoint, control),
            Input::Sdn(userdata, event) => self.on_sdn_event(now, userdata, event),
        }
    }

    fn on_shutdown(&mut self, _now: Instant) {}
}

impl<Endpoint: Debug + Copy + Clone + Hash + Eq> TaskSwitcherChild<Output<Endpoint>> for ClusterRoom<Endpoint> {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Metadata => {
                    if let Some(out) = self.metadata.pop_output(now, &mut self.switcher) {
                        match out {
                            metadata::Output::Kv(control) => break Some(Output::Sdn(RoomUserData(self.room, RoomFeature::MetaData), FeaturesControl::DhtKv(control))),
                            metadata::Output::Endpoint(endpoints, event) => break Some(Output::Endpoint(endpoints, event)),
                            metadata::Output::LastPeerLeaved => break Some(Output::Destroy(self.room)),
                        }
                    }
                }
                TaskType::MediaTrack => {
                    if let Some(out) = self.media_track.pop_output(now, &mut self.switcher) {
                        match out {
                            media_track::Output::Endpoint(endpoints, event) => break Some(Output::Endpoint(endpoints, event)),
                            media_track::Output::Pubsub(control) => break Some(Output::Sdn(RoomUserData(self.room, RoomFeature::MediaTrack), FeaturesControl::PubSub(control))),
                        }
                    }
                }
                TaskType::AudioMixer => {
                    if let Some(out) = self.audio_mixer.pop_output((), &mut self.switcher) {
                        match out {
                            audio_mixer::Output::Endpoint(endpoints, event) => break Some(Output::Endpoint(endpoints, event)),
                            audio_mixer::Output::Pubsub(control) => break Some(Output::Sdn(RoomUserData(self.room, RoomFeature::AudioMixer), FeaturesControl::PubSub(control))),
                        }
                    }
                }
            }
        }
    }
}

impl<Endpoint: Debug + Copy + Clone + Hash + Eq> ClusterRoom<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        let mixer_channel_id = (room.0 + 1).into(); //TODO generate this
        Self {
            room,
            metadata: TaskSwitcherBranch::new(RoomMetadata::new(room), TaskType::Metadata),
            media_track: TaskSwitcherBranch::new(MediaTrack::new(room), TaskType::MediaTrack),
            audio_mixer: TaskSwitcherBranch::new(AudioMixer::new(mixer_channel_id), TaskType::AudioMixer),
            switcher: TaskSwitcher::new(3),
            time_pivot: TimePivot::build(),
        }
    }

    fn on_sdn_event(&mut self, now: Instant, userdata: RoomUserData, event: FeaturesEvent) {
        match (userdata.1, event) {
            (RoomFeature::MetaData, FeaturesEvent::DhtKv(event)) => match event {
                dht_kv::Event::MapEvent(map, event) => self.metadata.input(&mut self.switcher).on_kv_event(map, event),
                dht_kv::Event::MapGetRes(_, _) => {}
            },
            (RoomFeature::MediaTrack, FeaturesEvent::PubSub(event)) => {
                self.media_track.input(&mut self.switcher).on_pubsub_event(event);
            }
            (RoomFeature::AudioMixer, FeaturesEvent::PubSub(event)) => {
                let now_ms = self.time_pivot.timestamp_ms(now);
                self.audio_mixer.input(&mut self.switcher).on_pubsub_event(now_ms, event);
            }
            _ => {}
        }
    }

    fn on_endpoint_control(&mut self, now: Instant, endpoint: Endpoint, control: ClusterEndpointControl) {
        match control {
            ClusterEndpointControl::Join(peer, meta, publish, subscribe, mixer) => {
                self.audio_mixer.input(&mut self.switcher).on_join(endpoint, peer.clone(), mixer);
                self.metadata.input(&mut self.switcher).on_join(endpoint, peer, meta, publish, subscribe);
            }
            ClusterEndpointControl::Leave => {
                self.audio_mixer.input(&mut self.switcher).on_leave(endpoint);
                self.metadata.input(&mut self.switcher).on_leave(endpoint);
            }
            ClusterEndpointControl::SubscribePeer(target) => {
                self.metadata.input(&mut self.switcher).on_subscribe_peer(endpoint, target);
            }
            ClusterEndpointControl::UnsubscribePeer(target) => {
                self.metadata.input(&mut self.switcher).on_unsubscribe_peer(endpoint, target);
            }
            ClusterEndpointControl::RemoteTrack(track, control) => self.on_control_remote_track(now, endpoint, track, control),
            ClusterEndpointControl::LocalTrack(track, control) => self.on_control_local_track(now, endpoint, track, control),
        }
    }
}

impl<Endpoint: Debug + Clone + Copy + Hash + Eq> ClusterRoom<Endpoint> {
    fn on_control_remote_track(&mut self, now: Instant, endpoint: Endpoint, track: RemoteTrackId, control: ClusterRemoteTrackControl) {
        match control {
            ClusterRemoteTrackControl::Started(name, meta) => {
                let peer = return_if_none!(self.metadata.get_peer_from_endpoint(endpoint));
                log::info!("[ClusterRoom {}] started track {:?}/{track} => {peer}/{name}", self.room, endpoint);

                if meta.kind.is_audio() {
                    let now_ms = self.time_pivot.timestamp_ms(now);
                    self.audio_mixer.input(&mut self.switcher).on_track_publish(now_ms, endpoint, track, peer.clone(), name.clone());
                }
                self.media_track.input(&mut self.switcher).on_track_publish(endpoint, track, peer, name.clone());
                self.metadata.input(&mut self.switcher).on_track_publish(endpoint, track, name, meta.clone());
            }
            ClusterRemoteTrackControl::Media(media) => {
                if media.meta.is_audio() {
                    let now_ms = self.time_pivot.timestamp_ms(now);
                    self.audio_mixer.input(&mut self.switcher).on_track_data(now_ms, endpoint, track, &media);
                }
                self.media_track.input(&mut self.switcher).on_track_data(endpoint, track, media);
            }
            ClusterRemoteTrackControl::Ended(_name, meta) => {
                log::info!("[ClusterRoom {}] stopped track {:?}/{track}", self.room, endpoint);

                if meta.kind.is_audio() {
                    let now_ms = self.time_pivot.timestamp_ms(now);
                    self.audio_mixer.input(&mut self.switcher).on_track_unpublish(now_ms, endpoint, track);
                }
                self.media_track.input(&mut self.switcher).on_track_unpublish(endpoint, track);
                self.metadata.input(&mut self.switcher).on_track_unpublish(endpoint, track);
            }
        }
    }

    fn on_control_local_track(&mut self, now: Instant, endpoint: Endpoint, track_id: LocalTrackId, control: ClusterLocalTrackControl) {
        match control {
            ClusterLocalTrackControl::Subscribe(target_peer, target_track) => self.media_track.input(&mut self.switcher).on_track_subscribe(endpoint, track_id, target_peer, target_track),
            ClusterLocalTrackControl::RequestKeyFrame => self.media_track.input(&mut self.switcher).on_track_request_key(endpoint, track_id),
            ClusterLocalTrackControl::DesiredBitrate(bitrate) => self.media_track.input(&mut self.switcher).on_track_desired_bitrate(now, endpoint, track_id, bitrate),
            ClusterLocalTrackControl::Unsubscribe => self.media_track.input(&mut self.switcher).on_track_unsubscribe(endpoint, track_id),
        }
    }
}

#[cfg(test)]
mod tests {
    //TODO join room should set key-value and SUB to maps
    //TODO maps event should fire event to endpoint
    //TODO leave room should del key-value
    //TODO track started should SET key-value and pubsub START
    //TODO track feedback should fire event to endpoint
    //TODO track stopped should DEL key-value and pubsub STOP
    //TODO subscribe track should SUB channel
    //TODO feddback track should FEEDBACK channel
    //TODO channel data should fire event to endpoint
    //TODO unsubscribe track should UNSUB channel
}
