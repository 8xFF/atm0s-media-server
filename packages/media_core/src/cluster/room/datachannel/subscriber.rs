use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId};
use derivative::Derivative;
use media_server_protocol::datachannel::DataChannelPacket;
use sans_io_runtime::{return_if_none, TaskSwitcherChild};

use crate::cluster::{id_generator, ClusterEndpointEvent, ClusterRoomHash};

use super::Output;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
struct ChannelContainer<Endpoint> {
    subscribers: HashMap<Endpoint, ()>,
    key: String,
}

pub struct DataChannelSubscriber<Endpoint> {
    room: ClusterRoomHash,
    channels: HashMap<ChannelId, ChannelContainer<Endpoint>>,
    subscribers: HashMap<Endpoint, Vec<ChannelId>>,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Hash + Eq + Copy + Debug> DataChannelSubscriber<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            queue: VecDeque::new(),
            channels: HashMap::new(),
            subscribers: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.channels.is_empty()
    }

    pub fn on_channel_subscribe(&mut self, endpoint: Endpoint, key: &str) {
        let channel_id: ChannelId = id_generator::gen_datachannel_id(self.room, key.to_string());
        log::info!("[ClusterRoomDataChannel {}/Subscribers] peer {:?} subscribe channel: {channel_id}", self.room, endpoint);
        let channel_container = self.channels.entry(channel_id).or_insert_with(|| {
            let channel = ChannelContainer {
                subscribers: HashMap::new(),
                key: key.to_string(),
            };
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart)));
            channel
        });

        if let Some(subscriber) = self.subscribers.get_mut(&endpoint) {
            if !channel_container.subscribers.contains_key(&endpoint) {
                subscriber.push(channel_id);
            }
        } else {
            self.subscribers.insert(endpoint, vec![channel_id]);
        }
        channel_container.subscribers.insert(endpoint, ());

        if channel_container.subscribers.len() == 1 {
            log::info!("[ClusterRoomDataChannel {}/Subscribers] first subscriber => Sub channel {channel_id}", self.room);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto)));
        }
    }

    pub fn on_channel_unsubscribe(&mut self, endpoint: Endpoint, key: &str) {
        let channel_id: ChannelId = id_generator::gen_datachannel_id(self.room, key.to_string());
        log::info!("[ClusterRoomDataChannel {}/Subscribers] peer {:?} unsubscribe channel: {channel_id}", self.room, endpoint);
        let channel_container = return_if_none!(self.channels.get_mut(&channel_id));
        if channel_container.subscribers.contains_key(&endpoint) {
            channel_container.subscribers.remove(&endpoint);
            if let Some(endpoint_subscriptions) = self.subscribers.get_mut(&endpoint) {
                if let Some(index) = endpoint_subscriptions.iter().position(|x| *x == channel_id) {
                    endpoint_subscriptions.swap_remove(index);
                }

                if endpoint_subscriptions.is_empty() {
                    self.subscribers.remove(&endpoint);
                }
            }
            if channel_container.subscribers.is_empty() {
                log::info!("[ClusterRoomDataChannel {}/Subscribers] last subscriber => Unsub channel {channel_id}", self.room);
                self.channels.remove(&channel_id);
                self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto)));
                self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop)));
                if self.channels.is_empty() {
                    log::info!("[ClusterRoomDataChannel {}/Subscribers] last channel => Stop channel {channel_id}", self.room);
                    self.queue.push_back(Output::OnResourceEmpty);
                }
            }
        } else {
            log::warn!("[ClusterRoomDataChannel {}/Subscribers] peer {:?} not subscribe in channel {channel_id}", self.room, endpoint);
        }
    }

    pub fn on_leave(&mut self, endpoint: Endpoint) {
        log::info!("[ClusterRoomDataChannel {}/Subscribers] peer {:?} leave", self.room, endpoint);
        let subscriber = return_if_none!(self.subscribers.remove(&endpoint));
        for channel_id in subscriber {
            if let Some(channel_container) = self.channels.get_mut(&channel_id) {
                channel_container.subscribers.remove(&endpoint);
                if channel_container.subscribers.is_empty() {
                    log::info!("[ClusterRoomDataChannel {}/Subscribers] last subscriber => Unsub channel {channel_id}", self.room);
                    self.channels.remove(&channel_id);
                    self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto)));
                    self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop)));
                }
                if self.channels.is_empty() {
                    log::info!("[ClusterRoomDataChannel {}/Subscribers] last channel => Stop channel {channel_id}", self.room);
                    self.queue.push_back(Output::OnResourceEmpty);
                }
            }
        }
    }

    pub fn on_channel_data(&mut self, channel_id: ChannelId, data: Vec<u8>) {
        log::info!("[ClusterRoomDataChannel {}/Subscribers] Receive data from channel {channel_id}", self.room);
        let pkt = return_if_none!(DataChannelPacket::deserialize(&data));
        if let Some(channel_container) = self.channels.get(&channel_id) {
            for endpoint in &channel_container.subscribers {
                self.queue.push_back(Output::Endpoint(
                    vec![*endpoint.0],
                    ClusterEndpointEvent::ChannelMessage(channel_container.key.clone(), pkt.from.clone(), pkt.data.clone()),
                ));
            }
        } else {
            log::warn!("[ClusterRoomDataChannel {}/Subscribers] Receive data from unknown channel {channel_id}", self.room);
        }
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for DataChannelSubscriber<Endpoint> {
    type Time = ();
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint> Drop for DataChannelSubscriber<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomDataChannel {}/Subscriber] Drop", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
        assert_eq!(self.channels.len(), 0, "Channels not empty on drop");
        assert_eq!(self.subscribers.len(), 0, "Subscribers not empty on drop");
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
        room::datachannel::{subscriber::DataChannelSubscriber, Output},
        ClusterEndpointEvent,
    };

    #[test]
    fn sub_unsub() {
        let now = ();
        let room = 1.into();
        let mut subscriber = DataChannelSubscriber::new(room);
        let endpoint1 = 1;
        let endpoint2 = 2;
        let endpoint3 = 3;
        let key = "test";
        let key2 = "test2";

        // 1 -> test
        // 2 -> test
        // 3 -> test2

        subscriber.on_channel_subscribe(endpoint1, key);
        let channel_id = id_generator::gen_datachannel_id(room, key.to_string());
        let channel_id2 = id_generator::gen_datachannel_id(room, key2.to_string());

        assert!(!subscriber.is_empty());
        // First subscriber will start publish and subscribe on pubsub channel
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(now), None);

        // Second subscriber will do nothing but register in the subscriber list
        subscriber.on_channel_subscribe(endpoint2, key);
        assert_eq!(subscriber.pop_output(now), None);

        // First subscriber of a new channel should start publish and subscribe too
        subscriber.on_channel_subscribe(endpoint3, key2);
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::PubStart))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::SubAuto))));

        subscriber.on_channel_unsubscribe(endpoint1, key);
        subscriber.on_channel_unsubscribe(endpoint2, key);

        // Last subscriber that unsubscribes will stop the channel
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop))));

        // Last channel that unsubscribes will stop publish and return empty resource
        subscriber.on_channel_unsubscribe(endpoint3, key2);
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::PubStop))));
        assert_eq!(subscriber.pop_output(now), Some(Output::OnResourceEmpty));

        assert!(subscriber.is_empty());
    }

    #[test]
    fn receive_data() {
        let now = ();
        let room = 1.into();
        let mut subscriber = DataChannelSubscriber::new(room);
        let endpoint1 = 1;
        let endpoint2 = 2;
        let endpoint3 = 3;
        let key = "test";
        let key_fake = "different_channel";

        let channel_id = id_generator::gen_datachannel_id(room, key.to_string());
        let channel_id_fake = id_generator::gen_datachannel_id(room, key_fake.to_string());

        subscriber.on_channel_subscribe(endpoint1, key);
        subscriber.on_channel_subscribe(endpoint2, key);
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(now), None);

        subscriber.on_channel_subscribe(endpoint3, key_fake);
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::PubStart))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(now), None);

        let peer_id = PeerId::from("testid");
        let pkt = DataChannelPacket {
            from: peer_id.clone(),
            data: vec![1, 2, 3],
        };
        subscriber.on_channel_data(channel_id, pkt.clone().serialize());
        let mut receivers = HashMap::new();
        receivers.insert(endpoint1, false);
        receivers.insert(endpoint2, false);
        receivers.insert(endpoint3, false);

        while let Some(out) = subscriber.pop_output(now) {
            match out {
                Output::Endpoint(endpoint, ClusterEndpointEvent::ChannelMessage(key, peer, data)) => {
                    assert_eq!(key, key);
                    assert_eq!(peer, peer_id);
                    assert_eq!(data, pkt.data);
                    assert!(receivers.contains_key(&endpoint[0]));
                    *receivers.get_mut(&endpoint[0]).unwrap() = true;
                }
                _ => panic!("Unexpected output: {:?}", out),
            }
        }

        // Every endpoint 1 and 2 should received the message
        // Endpoint 3 should not receive anything
        assert!(receivers[&endpoint1]);
        assert!(receivers[&endpoint2]);
        assert!(!receivers[&endpoint3]);
        assert_eq!(subscriber.pop_output(now), None);

        subscriber.on_channel_unsubscribe(endpoint1, key);
        subscriber.on_channel_unsubscribe(endpoint2, key);
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop))));
        subscriber.on_channel_unsubscribe(endpoint3, key_fake);
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id_fake, ChannelControl::PubStop))));

        assert_eq!(subscriber.pop_output(now), Some(Output::OnResourceEmpty));
    }

    #[test]
    fn leave_room() {
        let now = ();
        let room = 1.into();
        let mut subscriber = DataChannelSubscriber::new(room);
        let endpoint1 = 1;
        let endpoint2 = 2;
        let key = "test";

        let channel_id = id_generator::gen_datachannel_id(room, key.to_string());

        subscriber.on_channel_subscribe(endpoint1, key);
        subscriber.on_channel_subscribe(endpoint2, key);
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(now), None);

        subscriber.on_leave(endpoint1);
        assert_eq!(subscriber.pop_output(now), None);
        subscriber.on_leave(endpoint2);

        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop))));
        assert_eq!(subscriber.pop_output(now), Some(Output::OnResourceEmpty));
    }
}
