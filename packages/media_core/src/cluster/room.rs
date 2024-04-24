//!
//! This part is composer from some other small parts: Metadata, Channel Subscriber, Channel Publisher
//!
//! Main functions:
//!
//! - Send/Recv metadata related key-value
//! - Send/Receive pubsub channel
//!

use std::{collections::VecDeque, fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::features::{dht_kv, pubsub, FeaturesControl, FeaturesEvent};
use sans_io_runtime::{Task, TaskSwitcher};

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
    Destroy,
}

#[derive(num_enum::TryFromPrimitive)]
#[repr(usize)]
enum TaskType {
    Publisher,
    Subscriber,
    Metadata,
}

pub struct ClusterRoom<Owner> {
    room: ClusterRoomHash,
    metadata: RoomMetadata<Owner>,
    publisher: RoomChannelPublisher<Owner>,
    subscriber: RoomChannelSubscribe<Owner>,
    switcher: TaskSwitcher,
    queue: VecDeque<Output<Owner>>,
    destroyed: bool, //this flag for avoiding multi-time output destroy output
}

impl<Owner: Debug + Copy + Clone + Hash + Eq> Task<Input<Owner>, Output<Owner>> for ClusterRoom<Owner> {
    fn on_tick(&mut self, _now: Instant) -> Option<Output<Owner>> {
        None
    }

    fn on_event(&mut self, now: Instant, input: Input<Owner>) -> Option<Output<Owner>> {
        match input {
            Input::Endpoint(owner, control) => self.on_endpoint_control(now, owner, control),
            Input::Sdn(event) => self.on_sdn_event(now, event),
        }
    }

    fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        while let Some(c) = self.switcher.queue_current() {
            match c.try_into().ok()? {
                TaskType::Metadata => {
                    if let Some(out) = self.switcher.queue_process(self.metadata.pop_output(now)) {
                        return Some(self.process_meta_output(out));
                    }
                }
                TaskType::Publisher => {
                    if let Some(out) = self.switcher.queue_process(self.publisher.pop_output()) {
                        return Some(self.process_publisher_output(out));
                    }
                }
                TaskType::Subscriber => {
                    if let Some(out) = self.switcher.queue_process(self.subscriber.pop_output(now)) {
                        return Some(self.process_subscriber_output(out));
                    }
                }
            }
        }

        if self.metadata.peers() == 0 && !self.destroyed {
            log::info!("[ClusterRoom {}] leave last peer => remove room", self.room);
            self.destroyed = true;
            Some(Output::Destroy)
        } else {
            None
        }
    }

    fn shutdown(&mut self, _now: Instant) -> Option<Output<Owner>> {
        None
    }
}

impl<Owner: Debug + Copy + Clone + Hash + Eq> ClusterRoom<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            metadata: RoomMetadata::new(room),
            publisher: RoomChannelPublisher::new(room),
            subscriber: RoomChannelSubscribe::new(room),
            switcher: TaskSwitcher::new(3),
            queue: VecDeque::new(),
            destroyed: false,
        }
    }

    fn on_sdn_event(&mut self, _now: Instant, event: FeaturesEvent) -> Option<Output<Owner>> {
        match event {
            FeaturesEvent::DhtKv(event) => match event {
                dht_kv::Event::MapEvent(map, event) => {
                    let out = self.metadata.on_kv_event(map, event)?;
                    Some(self.process_meta_output(out))
                }
                dht_kv::Event::MapGetRes(_, _) => None,
            },
            FeaturesEvent::PubSub(pubsub::Event(channel, event)) => match event {
                pubsub::ChannelEvent::RouteChanged(next) => {
                    let out = self.subscriber.on_channel_relay_changed(channel, next)?;
                    Some(self.process_subscriber_output(out))
                }
                pubsub::ChannelEvent::SourceData(_, data) => {
                    let out = self.subscriber.on_channel_data(channel, data)?;
                    Some(self.process_subscriber_output(out))
                }
                pubsub::ChannelEvent::FeedbackData(fb) => {
                    let out = self.publisher.on_channel_feedback(channel, fb)?;
                    Some(self.process_publisher_output(out))
                }
            },
            _ => None,
        }
    }

    fn on_endpoint_control(&mut self, now: Instant, owner: Owner, control: ClusterEndpointControl) -> Option<Output<Owner>> {
        match control {
            ClusterEndpointControl::Join(peer, meta, publish, subscribe) => {
                let out = self.metadata.on_join(owner, peer, meta, publish, subscribe)?;
                Some(self.process_meta_output(out))
            }
            ClusterEndpointControl::Leave => {
                let out = self.metadata.on_leave(owner);
                Some(self.process_meta_output(out?))
            }
            ClusterEndpointControl::SubscribePeer(target) => {
                let out = self.metadata.on_subscribe_peer(owner, target)?;
                Some(self.process_meta_output(out))
            }
            ClusterEndpointControl::UnsubscribePeer(target) => {
                let out = self.metadata.on_unsubscribe_peer(owner, target)?;
                Some(self.process_meta_output(out))
            }
            ClusterEndpointControl::RemoteTrack(track, control) => self.control_remote_track(now, owner, track, control),
            ClusterEndpointControl::LocalTrack(track, control) => self.control_local_track(now, owner, track, control),
        }
    }
}

impl<Owner: Debug + Clone + Copy + Hash + Eq> ClusterRoom<Owner> {
    fn control_remote_track(&mut self, _now: Instant, owner: Owner, track: RemoteTrackId, control: ClusterRemoteTrackControl) -> Option<Output<Owner>> {
        match control {
            ClusterRemoteTrackControl::Started(name, meta) => {
                let peer = self.metadata.get_peer_from_owner(owner)?;
                if let Some(out) = self.publisher.on_track_publish(owner, track, peer, name.clone()) {
                    let out = self.process_publisher_output(out);
                    self.queue.push_back(out);
                }
                if let Some(out) = self.metadata.on_track_publish(owner, track, name.clone(), meta.clone()) {
                    let out = self.process_meta_output(out);
                    self.queue.push_back(out);
                }
                self.queue.pop_front()
            }
            ClusterRemoteTrackControl::Media(media) => {
                let out = self.publisher.on_track_data(owner, track, media)?;
                Some(self.process_publisher_output(out))
            }
            ClusterRemoteTrackControl::Ended => {
                if let Some(out) = self.publisher.on_track_unpublish(owner, track) {
                    let out = self.process_publisher_output(out);
                    self.queue.push_back(out);
                }
                if let Some(out) = self.metadata.on_track_unpublish(owner, track) {
                    let out = self.process_meta_output(out);
                    self.queue.push_back(out);
                }
                self.queue.pop_front()
            }
        }
    }

    fn control_local_track(&mut self, now: Instant, owner: Owner, track_id: LocalTrackId, control: ClusterLocalTrackControl) -> Option<Output<Owner>> {
        let out = match control {
            ClusterLocalTrackControl::Subscribe(target_peer, target_track) => self.subscriber.on_track_subscribe(owner, track_id, target_peer, target_track),
            ClusterLocalTrackControl::RequestKeyFrame => self.subscriber.on_track_request_key(owner, track_id),
            ClusterLocalTrackControl::DesiredBitrate(bitrate) => self.subscriber.on_track_desired_bitrate(now, owner, track_id, bitrate),
            ClusterLocalTrackControl::Unsubscribe => self.subscriber.on_track_unsubscribe(owner, track_id),
        }?;
        Some(self.process_subscriber_output(out))
    }

    fn process_meta_output(&mut self, out: metadata::Output<Owner>) -> Output<Owner> {
        self.switcher.queue_flag_task(TaskType::Metadata as usize);
        match out {
            metadata::Output::Kv(control) => Output::Sdn(self.room, FeaturesControl::DhtKv(control)),
            metadata::Output::Endpoint(owners, event) => Output::Endpoint(owners, event),
        }
    }

    fn process_publisher_output(&mut self, out: channel_pub::Output<Owner>) -> Output<Owner> {
        self.switcher.queue_flag_task(TaskType::Publisher as usize);
        match out {
            channel_pub::Output::Pubsub(control) => Output::Sdn(self.room, FeaturesControl::PubSub(control)),
            channel_pub::Output::Endpoint(owners, event) => Output::Endpoint(owners, event),
        }
    }

    fn process_subscriber_output(&mut self, out: channel_sub::Output<Owner>) -> Output<Owner> {
        self.switcher.queue_flag_task(TaskType::Subscriber as usize);
        match out {
            channel_sub::Output::Pubsub(control) => Output::Sdn(self.room, FeaturesControl::PubSub(control)),
            channel_sub::Output::Endpoint(owners, event) => Output::Endpoint(owners, event),
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
