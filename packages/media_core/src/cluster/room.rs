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

use atm0s_sdn::features::{dht_kv, FeaturesControl, FeaturesEvent};
use datachannel::RoomChannel;
use media_server_protocol::datachannel::DataChannelPacket;
use sans_io_runtime::{return_if_none, Task, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::transport::{LocalTrackId, RemoteTrackId};

use audio_mixer::AudioMixer;
use media_track::MediaTrack;
use metadata::RoomMetadata;

use super::{id_generator, ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackControl, ClusterRemoteTrackControl, ClusterRoomHash};

mod audio_mixer;
mod datachannel;
mod media_track;
mod metadata;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum RoomFeature {
    MetaData,
    MediaTrack,
    AudioMixer,
    DataChannel,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct RoomUserData(pub(crate) ClusterRoomHash, pub(crate) RoomFeature);

pub enum Input<Endpoint> {
    Sdn(RoomUserData, FeaturesEvent),
    Endpoint(Endpoint, ClusterEndpointControl),
}

#[derive(Debug, PartialEq, Eq)]
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
    DataChannel,
}

pub struct ClusterRoom<Endpoint: Debug + Copy + Clone + Hash + Eq> {
    room: ClusterRoomHash,
    metadata: TaskSwitcherBranch<RoomMetadata<Endpoint>, metadata::Output<Endpoint>>,
    media_track: TaskSwitcherBranch<MediaTrack<Endpoint>, media_track::Output<Endpoint>>,
    audio_mixer: TaskSwitcherBranch<AudioMixer<Endpoint>, audio_mixer::Output<Endpoint>>,
    datachannel: TaskSwitcherBranch<RoomChannel<Endpoint>, datachannel::Output<Endpoint>>,
    switcher: TaskSwitcher,
}

impl<Endpoint: Debug + Copy + Clone + Hash + Eq> Task<Input<Endpoint>, Output<Endpoint>> for ClusterRoom<Endpoint> {
    fn on_tick(&mut self, now: Instant) {
        self.audio_mixer.input(&mut self.switcher).on_tick(now);
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
    type Time = ();
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Metadata => {
                    if let Some(out) = self.metadata.pop_output((), &mut self.switcher) {
                        match out {
                            metadata::Output::Kv(control) => break Some(Output::Sdn(RoomUserData(self.room, RoomFeature::MetaData), FeaturesControl::DhtKv(control))),
                            metadata::Output::Endpoint(endpoints, event) => break Some(Output::Endpoint(endpoints, event)),
                            metadata::Output::OnResourceEmpty => {
                                if self.is_empty() {
                                    break Some(Output::Destroy(self.room));
                                }
                            }
                        }
                    }
                }
                TaskType::MediaTrack => {
                    if let Some(out) = self.media_track.pop_output((), &mut self.switcher) {
                        match out {
                            media_track::Output::Endpoint(endpoints, event) => break Some(Output::Endpoint(endpoints, event)),
                            media_track::Output::Pubsub(control) => break Some(Output::Sdn(RoomUserData(self.room, RoomFeature::MediaTrack), FeaturesControl::PubSub(control))),
                            media_track::Output::OnResourceEmpty => {
                                if self.is_empty() {
                                    break Some(Output::Destroy(self.room));
                                }
                            }
                        }
                    }
                }
                TaskType::AudioMixer => {
                    if let Some(out) = self.audio_mixer.pop_output((), &mut self.switcher) {
                        match out {
                            audio_mixer::Output::Endpoint(endpoints, event) => break Some(Output::Endpoint(endpoints, event)),
                            audio_mixer::Output::Pubsub(control) => break Some(Output::Sdn(RoomUserData(self.room, RoomFeature::AudioMixer), FeaturesControl::PubSub(control))),
                            audio_mixer::Output::OnResourceEmpty => {
                                if self.is_empty() {
                                    break Some(Output::Destroy(self.room));
                                }
                            }
                        }
                    }
                }
                TaskType::DataChannel => {
                    if let Some(out) = self.datachannel.pop_output((), &mut self.switcher) {
                        match out {
                            datachannel::Output::Endpoint(endpoints, event) => break Some(Output::Endpoint(endpoints, event)),
                            datachannel::Output::Pubsub(control) => break Some(Output::Sdn(RoomUserData(self.room, RoomFeature::DataChannel), FeaturesControl::PubSub(control))),
                            datachannel::Output::OnResourceEmpty => {
                                if self.is_empty() {
                                    break Some(Output::Destroy(self.room));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<Endpoint: Debug + Copy + Clone + Hash + Eq> ClusterRoom<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        let mixer_channel_id = id_generator::gen_mixer_auto_channel_id(room);
        Self {
            room,
            metadata: TaskSwitcherBranch::new(RoomMetadata::new(room), TaskType::Metadata),
            media_track: TaskSwitcherBranch::new(MediaTrack::new(room), TaskType::MediaTrack),
            audio_mixer: TaskSwitcherBranch::new(AudioMixer::new(room, mixer_channel_id), TaskType::AudioMixer),
            datachannel: TaskSwitcherBranch::new(RoomChannel::new(room), TaskType::DataChannel),
            switcher: TaskSwitcher::new(4),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.metadata.is_empty() && self.media_track.is_empty() && self.audio_mixer.is_empty()
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
                self.audio_mixer.input(&mut self.switcher).on_pubsub_event(now, event);
            }
            (RoomFeature::DataChannel, FeaturesEvent::PubSub(event)) => {
                self.datachannel.input(&mut self.switcher).on_pubsub_event(event);
            }
            _ => {}
        }
    }

    fn on_endpoint_control(&mut self, now: Instant, endpoint: Endpoint, control: ClusterEndpointControl) {
        match control {
            ClusterEndpointControl::Join(peer, meta, publish, subscribe, mixer) => {
                self.audio_mixer.input(&mut self.switcher).on_join(now, endpoint, peer.clone(), mixer);
                self.metadata.input(&mut self.switcher).on_join(endpoint, peer, meta, publish, subscribe);
            }
            ClusterEndpointControl::Leave => {
                self.audio_mixer.input(&mut self.switcher).on_leave(now, endpoint);
                self.metadata.input(&mut self.switcher).on_leave(endpoint);
            }
            ClusterEndpointControl::SubscribePeer(target) => {
                self.metadata.input(&mut self.switcher).on_subscribe_peer(endpoint, target);
            }
            ClusterEndpointControl::UnsubscribePeer(target) => {
                self.metadata.input(&mut self.switcher).on_unsubscribe_peer(endpoint, target);
            }
            ClusterEndpointControl::AudioMixer(control) => {
                self.audio_mixer.input(&mut self.switcher).on_control(now, endpoint, control);
            }
            ClusterEndpointControl::RemoteTrack(track, control) => self.on_control_remote_track(now, endpoint, track, control),
            ClusterEndpointControl::LocalTrack(track, control) => self.on_control_local_track(now, endpoint, track, control),
            ClusterEndpointControl::SubscribeChannel(key) => self.datachannel.input(&mut self.switcher).on_channel_subscribe(endpoint, &key),
            ClusterEndpointControl::PublishChannel(key, peer, message) => {
                let data_packet = DataChannelPacket { from: peer, data: message };
                self.datachannel.input(&mut self.switcher).on_channel_data(endpoint, &key, data_packet);
            }
            ClusterEndpointControl::UnsubscribeChannel(key) => self.datachannel.input(&mut self.switcher).on_channel_unsubscribe(endpoint, &key),
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
                    self.audio_mixer.input(&mut self.switcher).on_track_publish(now, endpoint, track, peer.clone(), name.clone());
                }
                self.media_track.input(&mut self.switcher).on_track_publish(endpoint, track, peer, name.clone());
                self.metadata.input(&mut self.switcher).on_track_publish(endpoint, track, name, meta.clone());
            }
            ClusterRemoteTrackControl::Media(media) => {
                if media.meta.is_audio() {
                    self.audio_mixer.input(&mut self.switcher).on_track_data(now, endpoint, track, &media);
                }
                self.media_track.input(&mut self.switcher).on_track_data(endpoint, track, media);
            }
            ClusterRemoteTrackControl::Ended(_name, meta) => {
                log::info!("[ClusterRoom {}] stopped track {:?}/{track}", self.room, endpoint);

                if meta.kind.is_audio() {
                    self.audio_mixer.input(&mut self.switcher).on_track_unpublish(now, endpoint, track);
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

impl<Endpoint: Debug + Copy + Clone + Hash + Eq> Drop for ClusterRoom<Endpoint> {
    fn drop(&mut self) {
        log::info!("Drop ClusterRoom {}", self.room);
        assert!(self.audio_mixer.is_empty(), "Audio mixer not empty");
        assert!(self.media_track.is_empty(), "Media track not empty");
        assert!(self.metadata.is_empty(), "Metadata not empty");
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use atm0s_sdn::features::{dht_kv, pubsub, FeaturesControl};
    use media_server_protocol::endpoint::{AudioMixerConfig, AudioMixerMode, PeerId, PeerMeta, RoomInfoPublish, RoomInfoSubscribe};
    use sans_io_runtime::{Task, TaskSwitcherChild};

    use crate::cluster::{id_generator, room::RoomFeature, ClusterEndpointControl, RoomUserData};

    use super::{ClusterRoom, Input, Output};

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

    #[test]
    fn cleanup_resource_sub_and_mixer() {
        let room_id = 0.into();
        let endpoint = 1;
        let peer: PeerId = "peer1".into();
        let t0 = Instant::now();
        let mut room = ClusterRoom::<u8>::new(room_id);
        room.on_event(
            t0,
            Input::Endpoint(
                endpoint,
                ClusterEndpointControl::Join(
                    peer.clone(),
                    PeerMeta { metadata: None },
                    RoomInfoPublish { peer: false, tracks: false },
                    RoomInfoSubscribe { peers: true, tracks: true },
                    Some(AudioMixerConfig {
                        mode: AudioMixerMode::Auto,
                        outputs: vec![0.into(), 1.into(), 2.into()],
                        sources: vec![],
                    }),
                ),
            ),
        );

        let room_peers_map = id_generator::peers_map(room_id);
        let room_tracks_map = id_generator::tracks_map(room_id);
        let room_mixer_auto_channel = id_generator::gen_mixer_auto_channel_id(room_id);

        assert_eq!(
            room.pop_output(()),
            Some(Output::Sdn(
                RoomUserData(room_id, RoomFeature::MetaData),
                FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, dht_kv::MapControl::Sub))
            ))
        );
        assert_eq!(
            room.pop_output(()),
            Some(Output::Sdn(
                RoomUserData(room_id, RoomFeature::MetaData),
                FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_tracks_map, dht_kv::MapControl::Sub))
            ))
        );
        assert_eq!(
            room.pop_output(()),
            Some(Output::Sdn(
                RoomUserData(room_id, RoomFeature::AudioMixer),
                FeaturesControl::PubSub(pubsub::Control(room_mixer_auto_channel, pubsub::ChannelControl::SubAuto))
            ))
        );
        assert_eq!(room.pop_output(()), None);

        //after leave we should auto cleanup all resources like kv, pubsub
        room.on_event(t0, Input::Endpoint(endpoint, ClusterEndpointControl::Leave));
        assert_eq!(
            room.pop_output(()),
            Some(Output::Sdn(
                RoomUserData(room_id, RoomFeature::MetaData),
                FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, dht_kv::MapControl::Unsub))
            ))
        );
        assert_eq!(
            room.pop_output(()),
            Some(Output::Sdn(
                RoomUserData(room_id, RoomFeature::MetaData),
                FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_tracks_map, dht_kv::MapControl::Unsub))
            ))
        );
        assert_eq!(
            room.pop_output(()),
            Some(Output::Sdn(
                RoomUserData(room_id, RoomFeature::AudioMixer),
                FeaturesControl::PubSub(pubsub::Control(room_mixer_auto_channel, pubsub::ChannelControl::UnsubAuto))
            ))
        );
        assert_eq!(room.pop_output(()), Some(Output::Destroy(room_id)));
        assert_eq!(room.pop_output(()), None);
    }
}
