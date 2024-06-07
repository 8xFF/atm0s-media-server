use std::{fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::features::pubsub;
use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    media::MediaPacket,
};
use publisher::RoomChannelPublisher;
use sans_io_runtime::{TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};

use crate::{
    cluster::{ClusterEndpointEvent, ClusterRoomHash},
    transport::{LocalTrackId, RemoteTrackId},
};

use self::subscriber::RoomChannelSubscribe;

pub mod publisher;
pub mod subscriber;

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
    OnResourceEmpty,
}

pub struct MediaTrack<Endpoint> {
    room: ClusterRoomHash,
    publisher: TaskSwitcherBranch<RoomChannelPublisher<Endpoint>, Output<Endpoint>>,
    subscriber: TaskSwitcherBranch<RoomChannelSubscribe<Endpoint>, Output<Endpoint>>,
    switcher: TaskSwitcher,
}

impl<Endpoint: Debug + Hash + Eq + Copy> MediaTrack<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            publisher: TaskSwitcherBranch::new(RoomChannelPublisher::new(room), TaskType::Publisher),
            subscriber: TaskSwitcherBranch::new(RoomChannelSubscribe::new(room), TaskType::Subscriber),
            switcher: TaskSwitcher::new(2),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.publisher.is_empty() && self.subscriber.is_empty()
    }

    pub fn on_pubsub_event(&mut self, event: pubsub::Event) {
        let channel = event.0;
        match event.1 {
            pubsub::ChannelEvent::RouteChanged(next) => {
                self.subscriber.input(&mut self.switcher).on_track_relay_changed(channel, next);
            }
            pubsub::ChannelEvent::SourceData(_, data) => {
                self.subscriber.input(&mut self.switcher).on_track_data(channel, data);
            }
            pubsub::ChannelEvent::FeedbackData(fb) => {
                self.publisher.input(&mut self.switcher).on_track_feedback(channel, fb);
            }
        }
    }

    pub fn on_track_publish(&mut self, endpoint: Endpoint, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        self.publisher.input(&mut self.switcher).on_track_publish(endpoint, track, peer, name);
    }

    pub fn on_track_data(&mut self, endpoint: Endpoint, track: RemoteTrackId, media: MediaPacket) {
        self.publisher.input(&mut self.switcher).on_track_data(endpoint, track, media);
    }

    pub fn on_track_unpublish(&mut self, endpoint: Endpoint, track: RemoteTrackId) {
        self.publisher.input(&mut self.switcher).on_track_unpublish(endpoint, track);
    }

    pub fn on_track_subscribe(&mut self, endpoint: Endpoint, track: LocalTrackId, target_peer: PeerId, target_track: TrackName) {
        self.subscriber.input(&mut self.switcher).on_track_subscribe(endpoint, track, target_peer, target_track);
    }

    pub fn on_track_request_key(&mut self, endpoint: Endpoint, track: LocalTrackId) {
        self.subscriber.input(&mut self.switcher).on_track_request_key(endpoint, track);
    }

    pub fn on_track_desired_bitrate(&mut self, now: Instant, endpoint: Endpoint, track: LocalTrackId, bitrate: u64) {
        self.subscriber.input(&mut self.switcher).on_track_desired_bitrate(now, endpoint, track, bitrate);
    }

    pub fn on_track_unsubscribe(&mut self, endpoint: Endpoint, track: LocalTrackId) {
        self.subscriber.input(&mut self.switcher).on_track_unsubscribe(endpoint, track);
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for MediaTrack<Endpoint> {
    type Time = Instant;

    fn pop_output(&mut self, now: Self::Time) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Publisher => {
                    if let Some(out) = self.publisher.pop_output(now, &mut self.switcher) {
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
                    if let Some(out) = self.subscriber.pop_output(now, &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            if self.is_empty() {
                                return Some(Output::OnResourceEmpty);
                            }
                        } else {
                            return Some(out);
                        }
                    }
                }
            }
        }
    }
}

impl<Endpoint> Drop for MediaTrack<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomMediaTrack] Drop {}", self.room);
    }
}
