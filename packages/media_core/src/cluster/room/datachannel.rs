use std::{fmt::Debug, hash::Hash};

use atm0s_sdn::features::pubsub::{self};
use media_server_protocol::datachannel::DataChannelPacket;
use publisher::DataChannelPublisher;
use sans_io_runtime::{TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};
use subscriber::DataChannelSubscriber;

use crate::cluster::{ClusterEndpointEvent, ClusterRoomHash};

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
    Pubsub(pubsub::Control),
    Endpoint(Vec<Endpoint>, ClusterEndpointEvent),
    OnResourceEmpty,
}

pub struct RoomChannel<Endpoint> {
    room: ClusterRoomHash,
    publisher: TaskSwitcherBranch<DataChannelPublisher<Endpoint>, Output<Endpoint>>,
    subscriber: TaskSwitcherBranch<DataChannelSubscriber<Endpoint>, Output<Endpoint>>,
    switcher: TaskSwitcher,
}

impl<Endpoint: Hash + Eq + Copy + Debug> RoomChannel<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        log::info!("[ClusterRoomMetadata] Create {}", room);
        Self {
            room,
            publisher: TaskSwitcherBranch::new(DataChannelPublisher::new(room), TaskType::Publisher),
            subscriber: TaskSwitcherBranch::new(DataChannelSubscriber::new(room), TaskType::Subscriber),
            switcher: TaskSwitcher::new(2),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.publisher.is_empty() && self.subscriber.is_empty()
    }

    pub fn on_pubsub_event(&mut self, event: pubsub::Event) {
        let channel = event.0;
        match event.1 {
            pubsub::ChannelEvent::SourceData(_, data) => {
                self.subscriber.input(&mut self.switcher).on_channel_data(channel, data);
            }
            _ => {}
        }
    }

    pub fn on_channel_data(&mut self, key: &str, data: DataChannelPacket) {
        self.publisher.input(&mut self.switcher).on_channel_data(key, data);
    }

    pub fn on_channel_subscribe(&mut self, endpoint: Endpoint, key: &str) {
        self.subscriber.input(&mut self.switcher).on_channel_subscribe(endpoint, key);
    }

    pub fn on_channel_unsubscribe(&mut self, endpoint: Endpoint, key: &str) {
        self.subscriber.input(&mut self.switcher).on_channel_unsubscribe(endpoint, key);
    }

    pub fn on_leave(&mut self, endpoint: Endpoint) {
        self.subscriber.input(&mut self.switcher).on_leave(endpoint);
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for RoomChannel<Endpoint> {
    type Time = ();

    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Publisher => {
                    if let Some(out) = self.publisher.pop_output((), &mut self.switcher) {
                        log::info!("[ClusterRoomDataChannel] poped Output publisher {:?}", out);
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
                        log::info!("[ClusterRoomDataChannel] poped Output Subscriber {:?}", out);
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

impl<Endpoint> Drop for RoomChannel<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomDataChannel] Drop {}", self.room);
    }
}
