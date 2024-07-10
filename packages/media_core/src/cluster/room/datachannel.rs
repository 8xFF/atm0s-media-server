use atm0s_sdn::features::pubsub::{self, ChannelId};
use media_server_protocol::datachannel::DataChannelPacket;
use publisher::DataChannelPublisher;
use sans_io_runtime::{TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
};
use subscriber::DataChannelSubscriber;

use crate::cluster::{id_generator, ClusterEndpointEvent, ClusterRoomHash};

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

struct ChannelContainer<Endpoint> {
    subscribers: HashSet<Endpoint>,
    key: String,
}

pub struct RoomChannel<Endpoint> {
    room: ClusterRoomHash,
    channels: HashMap<ChannelId, ChannelContainer<Endpoint>>,
    publisher: TaskSwitcherBranch<DataChannelPublisher<Endpoint>, Output<Endpoint>>,
    subscriber: TaskSwitcherBranch<DataChannelSubscriber<Endpoint>, Output<Endpoint>>,
    switcher: TaskSwitcher,
}

impl<Endpoint: Hash + Eq + Copy + Debug> RoomChannel<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        log::info!("[ClusterRoomDataChannel {}] Create virtual datachannel", room);
        Self {
            room,
            channels: HashMap::new(),
            publisher: TaskSwitcherBranch::new(DataChannelPublisher::new(room), TaskType::Publisher),
            subscriber: TaskSwitcherBranch::new(DataChannelSubscriber::new(room), TaskType::Subscriber),
            switcher: TaskSwitcher::new(2),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.publisher.is_empty() && self.subscriber.is_empty() && self.channels.is_empty()
    }

    pub fn on_pubsub_event(&mut self, event: pubsub::Event) {
        let channel_id = event.0;
        match event.1 {
            pubsub::ChannelEvent::SourceData(_, data) => {
                if let Some(channel) = self.channels.get(&channel_id) {
                    log::info!("[ClusterRoomDataChannel {}] Got message from channel {channel_id}, try publish to subscribers", self.room);
                    let endpoints: Vec<Endpoint> = channel.subscribers.iter().copied().collect();
                    self.subscriber.input(&mut self.switcher).on_channel_data(channel.key.clone(), endpoints, data);
                } else {
                    log::warn!("[ClusterRoomDataChannel {}] Unexpected Channel {}", self.room, channel_id);
                }
            }
            _ => {}
        }
    }

    pub fn on_channel_data(&mut self, key: &str, data: DataChannelPacket) {
        let channel_id: ChannelId = id_generator::gen_data_channel_id(self.room, key.to_string());
        self.publisher.input(&mut self.switcher).on_channel_data(channel_id, data);
    }

    pub fn on_channel_subscribe(&mut self, endpoint: Endpoint, key: &str) {
        log::info!("[ClusterRoomDataChannel {}] Endpoint {:?} Subscribe Channel {key}", self.room, endpoint);
        let channel_id: ChannelId = id_generator::gen_data_channel_id(self.room, key.to_string());
        if let Some(channel) = self.channels.get_mut(&channel_id) {
            if !channel.subscribers.insert(endpoint) {
                log::warn!("[ClusterRoomDataChannel {}] Endpoint {:?} already subscribed to Channel {}", self.room, endpoint, channel_id);
                return;
            }
            self.subscriber.input(&mut self.switcher).on_channel_subscribe(endpoint, channel_id);
        } else {
            log::info!("[ClusterRoomDataChannel {}] Create new Channel {}", self.room, channel_id);
            let mut channel = ChannelContainer {
                subscribers: HashSet::new(),
                key: key.to_string(),
            };
            channel.subscribers.insert(endpoint);
            self.channels.insert(channel_id, channel);
            self.publisher.input(&mut self.switcher).on_channel_create(channel_id);
            self.subscriber.input(&mut self.switcher).on_channel_create(endpoint, channel_id);
        }
    }

    pub fn on_channel_unsubscribe(&mut self, endpoint: Endpoint, key: &str) {
        log::info!("[ClusterRoomDataChannel {}] Endpoint {:?} Unsubscribe Channel {key}", self.room, endpoint);
        let channel_id: ChannelId = id_generator::gen_data_channel_id(self.room, key.to_string());
        self.unsub_channel(endpoint, channel_id);
    }

    fn unsub_channel(&mut self, endpoint: Endpoint, channel_id: ChannelId) {
        if let Some(channel) = self.channels.get_mut(&channel_id) {
            if !channel.subscribers.remove(&endpoint) {
                log::warn!("[ClusterRoomDataChannel {}] Endpoint {:?} not subscribed to Channel {}", self.room, endpoint, channel_id);
                return;
            }
            self.subscriber.input(&mut self.switcher).on_channel_unsubscribe(endpoint, channel_id);
            if channel.subscribers.is_empty() {
                log::info!("[ClusterRoomDataChannel {}] Channel have no subscribers, remove Channel {}", self.room, channel_id);
                self.publisher.input(&mut self.switcher).on_channel_close(channel_id);
                self.subscriber.input(&mut self.switcher).on_channel_close(channel_id);
                self.channels.remove(&channel_id);
            }
        }
    }

    pub fn on_leave(&mut self, endpoint: Endpoint) {
        for channel_id in self.subscriber.get_subscriptions(endpoint) {
            self.unsub_channel(endpoint, channel_id);
        }
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for RoomChannel<Endpoint> {
    type Time = ();

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
            }
        }
    }
}

impl<Endpoint> Drop for RoomChannel<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomDataChannel] Drop {}", self.room);
        assert!(self.channels.is_empty(), "channels should be empty on drop");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use atm0s_sdn::features::pubsub::{self, ChannelControl};
    use media_server_protocol::{datachannel::DataChannelPacket, endpoint::PeerId};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::cluster::{
        id_generator,
        room::datachannel::{Output, RoomChannel},
        ClusterEndpointEvent,
    };

    #[test]
    fn sub_unsub() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomChannel::new(room_id);
        let endpoint1 = 1;
        let endpoint2 = 2;
        let endpoint3 = 3;
        let key = "test";
        let key2 = "test2";

        // 1 -> test
        // 2 -> test
        // 3 -> test2
        let channel_id = id_generator::gen_data_channel_id(room_id, key.to_string());
        let channel_id2 = id_generator::gen_data_channel_id(room_id, key2.to_string());

        // First subscriber will start publish and subscribe on pubsub channel
        room.on_channel_subscribe(endpoint1, key);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(room.pop_output(now), None);

        // Second subscriber will do nothing but register in the subscriber list
        room.on_channel_subscribe(endpoint2, key);
        assert_eq!(room.pop_output(now), None);
        assert_eq!(room.subscriber.get_subscriptions(endpoint1), vec![channel_id]);
        assert_eq!(room.subscriber.get_subscriptions(endpoint2), vec![channel_id]);

        // First subscriber of a new channel should start publish and subscribe too
        room.on_channel_subscribe(endpoint3, key2);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::PubStart))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::SubAuto))));
        assert_eq!(room.subscriber.get_subscriptions(endpoint3), vec![channel_id2]);

        // Last subscriber that unsubscribes will stop the channel
        room.on_channel_unsubscribe(endpoint1, key);
        assert_eq!(room.subscriber.get_subscriptions(endpoint1), vec![]);
        room.on_channel_unsubscribe(endpoint2, key);
        assert_eq!(room.subscriber.get_subscriptions(endpoint2), vec![]);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto))));

        room.on_channel_unsubscribe(endpoint3, key2);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::PubStop))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);

        assert!(room.subscriber.is_empty());
    }

    #[test]
    fn receive_data() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomChannel::new(room_id);
        let endpoint1 = 1;
        let endpoint2 = 2;
        let endpoint3 = 3;
        let key = "test";
        let key_fake = "different_channel";

        let channel_id = id_generator::gen_data_channel_id(room_id, key.to_string());
        let channel_id_fake = id_generator::gen_data_channel_id(room_id, key_fake.to_string());

        room.on_channel_subscribe(endpoint1, key);
        room.on_channel_subscribe(endpoint2, key);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(room.pop_output(now), None);

        room.on_channel_subscribe(endpoint3, key_fake);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::PubStart))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::SubAuto))));
        assert_eq!(room.pop_output(now), None);

        let peer_id = PeerId::from("testid");
        let pkt = DataChannelPacket {
            from: peer_id.clone(),
            data: vec![1, 2, 3],
        };
        room.on_pubsub_event(pubsub::Event(channel_id, pubsub::ChannelEvent::SourceData(1, pkt.serialize())));
        let mut receivers = HashMap::new();
        receivers.insert(endpoint1, false);
        receivers.insert(endpoint2, false);
        receivers.insert(endpoint3, false);

        if let Some(out) = room.pop_output(now) {
            match out {
                Output::Endpoint(endpoints, ClusterEndpointEvent::VirtualChannelMessage(key, peer, data)) => {
                    assert_eq!(key, key);
                    assert_eq!(peer, peer_id);
                    assert_eq!(data, pkt.data);
                    for endpoint in endpoints {
                        *receivers.get_mut(&endpoint).unwrap() = true;
                    }
                }
                _ => panic!("Unexpected output: {:?}", out),
            }
        }

        // Every endpoint 1 and 2 should received the message
        // Endpoint 3 should not receive anything
        assert!(receivers[&endpoint1]);
        assert!(receivers[&endpoint2]);
        assert!(!receivers[&endpoint3]);
        assert_eq!(room.pop_output(now), None);

        room.on_channel_unsubscribe(endpoint1, key);
        room.on_channel_unsubscribe(endpoint2, key);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto))));

        room.on_channel_unsubscribe(endpoint3, key_fake);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::PubStop))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::UnsubAuto))));

        assert_eq!(room.pop_output(now), None);
    }

    #[test]
    fn leave_room() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomChannel::new(room_id);
        let endpoint1 = 1;
        let endpoint2 = 2;
        let key = "test";

        let channel_id = id_generator::gen_data_channel_id(room_id, key.to_string());

        room.on_channel_subscribe(endpoint1, key);
        room.on_channel_subscribe(endpoint2, key);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(room.pop_output(now), None);

        room.on_leave(endpoint1);
        assert_eq!(room.pop_output(now), None);
        room.on_leave(endpoint2);

        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop))));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);
    }
}
