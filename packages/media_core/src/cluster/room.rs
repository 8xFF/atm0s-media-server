//!
//! This part is composer from some other small parts: Metadata, Channel Subscriber, Channel Publisher
//!
//! Main functions:
//!
//! - Send/Recv metadata related key-value
//! - Send/Receive pubsub channel
//!

use std::{fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::features::{dht_kv, pubsub, FeaturesControl, FeaturesEvent};
use sans_io_runtime::{collections::DynamicDeque, return_if_none, return_if_some, Task, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::transport::{LocalTrackId, RemoteTrackId};

use self::{channel_pub::RoomChannelPublisher, channel_sub::RoomChannelSubscribe, metadata::RoomMetadata};

use super::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackControl, ClusterRemoteTrackControl, ClusterRoomHash};

mod channel_pub;
mod channel_sub;
mod metadata;

pub enum Input<Owner> {
    Sdn(FeaturesEvent),
    Endpoint(Owner, ClusterEndpointControl),
}

pub enum Output<Owner> {
    Sdn(ClusterRoomHash, FeaturesControl),
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    Destroy(ClusterRoomHash),
}

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(usize)]
enum TaskType {
    Publisher,
    Subscriber,
    Metadata,
}

pub struct ClusterRoom<Owner> {
    room: ClusterRoomHash,
    metadata: TaskSwitcherBranch<RoomMetadata<Owner>, metadata::Output<Owner>>,
    publisher: TaskSwitcherBranch<RoomChannelPublisher<Owner>, channel_pub::Output<Owner>>,
    subscriber: TaskSwitcherBranch<RoomChannelSubscribe<Owner>, channel_sub::Output<Owner>>,
    switcher: TaskSwitcher,
    queue: DynamicDeque<Output<Owner>, 64>,
}

impl<Owner: Debug + Copy + Clone + Hash + Eq> Task<Input<Owner>, Output<Owner>> for ClusterRoom<Owner> {
    fn on_tick(&mut self, _now: Instant) {}

    fn on_event(&mut self, now: Instant, input: Input<Owner>) {
        match input {
            Input::Endpoint(owner, control) => self.on_endpoint_control(now, owner, control),
            Input::Sdn(event) => self.on_sdn_event(now, event),
        }
    }

    fn on_shutdown(&mut self, _now: Instant) {}
}

impl<Owner: Debug + Copy + Clone + Hash + Eq> TaskSwitcherChild<Output<Owner>> for ClusterRoom<Owner> {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        return_if_some!(self.queue.pop_front());

        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Metadata => self.pop_meta_output(now),
                TaskType::Publisher => self.pop_publisher_output(now),
                TaskType::Subscriber => self.pop_subscriber_output(now),
            }

            return_if_some!(self.queue.pop_front());
        }
    }
}

impl<Owner: Debug + Copy + Clone + Hash + Eq> ClusterRoom<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            metadata: TaskSwitcherBranch::new(RoomMetadata::new(room), TaskType::Metadata),
            publisher: TaskSwitcherBranch::new(RoomChannelPublisher::new(room), TaskType::Publisher),
            subscriber: TaskSwitcherBranch::new(RoomChannelSubscribe::new(room), TaskType::Subscriber),
            switcher: TaskSwitcher::new(3),
            queue: DynamicDeque::default(),
        }
    }

    fn on_sdn_event(&mut self, _now: Instant, event: FeaturesEvent) {
        match event {
            FeaturesEvent::DhtKv(event) => match event {
                dht_kv::Event::MapEvent(map, event) => self.metadata.input(&mut self.switcher).on_kv_event(map, event),
                dht_kv::Event::MapGetRes(_, _) => {}
            },
            FeaturesEvent::PubSub(pubsub::Event(channel, event)) => match event {
                pubsub::ChannelEvent::RouteChanged(next) => {
                    self.subscriber.input(&mut self.switcher).on_channel_relay_changed(channel, next);
                }
                pubsub::ChannelEvent::SourceData(_, data) => {
                    self.subscriber.input(&mut self.switcher).on_channel_data(channel, data);
                }
                pubsub::ChannelEvent::FeedbackData(fb) => {
                    self.publisher.input(&mut self.switcher).on_channel_feedback(channel, fb);
                }
            },
            _ => {}
        }
    }

    fn on_endpoint_control(&mut self, now: Instant, owner: Owner, control: ClusterEndpointControl) {
        match control {
            ClusterEndpointControl::Join(peer, meta, publish, subscribe) => {
                self.metadata.input(&mut self.switcher).on_join(owner, peer, meta, publish, subscribe);
            }
            ClusterEndpointControl::Leave => {
                self.metadata.input(&mut self.switcher).on_leave(owner);
            }
            ClusterEndpointControl::SubscribePeer(target) => {
                self.metadata.input(&mut self.switcher).on_subscribe_peer(owner, target);
            }
            ClusterEndpointControl::UnsubscribePeer(target) => {
                self.metadata.input(&mut self.switcher).on_unsubscribe_peer(owner, target);
            }
            ClusterEndpointControl::RemoteTrack(track, control) => self.on_control_remote_track(now, owner, track, control),
            ClusterEndpointControl::LocalTrack(track, control) => self.on_control_local_track(now, owner, track, control),
        }
    }
}

impl<Owner: Debug + Clone + Copy + Hash + Eq> ClusterRoom<Owner> {
    fn on_control_remote_track(&mut self, _now: Instant, owner: Owner, track: RemoteTrackId, control: ClusterRemoteTrackControl) {
        match control {
            ClusterRemoteTrackControl::Started(name, meta) => {
                let peer = return_if_none!(self.metadata.get_peer_from_owner(owner));
                log::info!("[ClusterRoom {}] started track {:?}/{track} => {peer}/{name}", self.room, owner);

                self.publisher.input(&mut self.switcher).on_track_publish(owner, track, peer, name.clone());
                self.metadata.input(&mut self.switcher).on_track_publish(owner, track, name.clone(), meta.clone());
            }
            ClusterRemoteTrackControl::Media(media) => {
                self.publisher.input(&mut self.switcher).on_track_data(owner, track, media);
            }
            ClusterRemoteTrackControl::Ended => {
                log::info!("[ClusterRoom {}] stopped track {:?}/{track}", self.room, owner);
                self.publisher.input(&mut self.switcher).on_track_unpublish(owner, track);
                self.metadata.input(&mut self.switcher).on_track_unpublish(owner, track);
            }
        }
    }

    fn on_control_local_track(&mut self, now: Instant, owner: Owner, track_id: LocalTrackId, control: ClusterLocalTrackControl) {
        match control {
            ClusterLocalTrackControl::Subscribe(target_peer, target_track) => self.subscriber.input(&mut self.switcher).on_track_subscribe(owner, track_id, target_peer, target_track),
            ClusterLocalTrackControl::RequestKeyFrame => self.subscriber.input(&mut self.switcher).on_track_request_key(owner, track_id),
            ClusterLocalTrackControl::DesiredBitrate(bitrate) => self.subscriber.input(&mut self.switcher).on_track_desired_bitrate(now, owner, track_id, bitrate),
            ClusterLocalTrackControl::Unsubscribe => self.subscriber.input(&mut self.switcher).on_track_unsubscribe(owner, track_id),
        }
    }

    fn pop_meta_output(&mut self, now: Instant) {
        let out = return_if_none!(self.metadata.pop_output(now, &mut self.switcher));
        let out = match out {
            metadata::Output::Kv(control) => Output::Sdn(self.room, FeaturesControl::DhtKv(control)),
            metadata::Output::Endpoint(owners, event) => Output::Endpoint(owners, event),
            metadata::Output::LastPeerLeaved => Output::Destroy(self.room),
        };
        self.queue.push_back(out);
    }

    fn pop_publisher_output(&mut self, now: Instant) {
        let out = return_if_none!(self.publisher.pop_output(now, &mut self.switcher));
        let out = match out {
            channel_pub::Output::Pubsub(control) => Output::Sdn(self.room, FeaturesControl::PubSub(control)),
            channel_pub::Output::Endpoint(owners, event) => Output::Endpoint(owners, event),
        };
        self.queue.push_back(out);
    }

    fn pop_subscriber_output(&mut self, now: Instant) {
        let out = return_if_none!(self.subscriber.pop_output(now, &mut self.switcher));
        let out = match out {
            channel_sub::Output::Pubsub(control) => Output::Sdn(self.room, FeaturesControl::PubSub(control)),
            channel_sub::Output::Endpoint(owners, event) => Output::Endpoint(owners, event),
        };
        self.queue.push_back(out);
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
