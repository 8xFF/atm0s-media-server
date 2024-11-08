use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId};
use media_server_protocol::message_channel::MessageChannelPacket;
use media_server_utils::Count;
use sans_io_runtime::{return_if_none, TaskSwitcherChild};

use super::Output;
use crate::{
    cluster::{id_generator, ClusterEndpointEvent, ClusterRoomHash},
    endpoint::MessageChannelLabel,
};

struct ChannelContainer<Endpoint> {
    subscribers: HashSet<Endpoint>,
    label: MessageChannelLabel,
}

pub struct MessageChannelSubscriber<Endpoint> {
    _c: Count<Self>,
    room: ClusterRoomHash,
    channels: HashMap<ChannelId, ChannelContainer<Endpoint>>,
    subscriptions: HashMap<Endpoint, HashSet<ChannelId>>,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Hash + Eq + Copy + Debug> MessageChannelSubscriber<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            _c: Default::default(),
            room,
            queue: VecDeque::new(),
            channels: HashMap::new(),
            subscriptions: HashMap::new(),
        }
    }

    pub fn on_channel_subscribe(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        log::info!("[ClusterRoomDataChannel {}/Subscribers] Subscribe channel", self.room);

        let channel_id: ChannelId = id_generator::gen_msg_channel_id(self.room, label);

        match self.channels.entry(channel_id) {
            Entry::Occupied(mut o) => {
                o.get_mut().subscribers.insert(endpoint);
            }
            Entry::Vacant(v) => {
                let mut channel = ChannelContainer {
                    subscribers: HashSet::new(),
                    label: label.clone(),
                };
                channel.subscribers.insert(endpoint);
                v.insert(channel);
                self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto)));
            }
        }

        self.subscriptions.entry(endpoint).or_default().insert(channel_id);
    }

    pub fn on_channel_unsubscribe(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        log::info!("[ClusterRoomDataChannel {}/Subscribers] unsubscribe channel", self.room);
        let channel_id: ChannelId = id_generator::gen_msg_channel_id(self.room, label);

        let channel = return_if_none!(self.channels.get_mut(&channel_id));

        channel.subscribers.remove(&endpoint);
        if let Some(channels) = self.subscriptions.get_mut(&endpoint) {
            channels.remove(&channel_id);
            if channels.is_empty() {
                self.subscriptions.remove(&endpoint);
            }
        }

        if channel.subscribers.is_empty() {
            self.channels.remove(&channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto)));
        }
    }

    pub fn on_channel_data(&mut self, channel_id: ChannelId, data: Vec<u8>) {
        log::info!("[ClusterRoomDataChannel {}/Subscribers] on received data from cluster", self.room);
        let channel = return_if_none!(self.channels.get_mut(&channel_id));

        let endpoints = channel.subscribers.iter().cloned().collect();

        let pkt = return_if_none!(MessageChannelPacket::deserialize(&data));
        self.queue.push_back(Output::Endpoint(
            endpoints,
            ClusterEndpointEvent::MessageChannelData(channel.label.clone(), pkt.from.clone(), pkt.data.clone()),
        ));
    }

    pub fn on_leave(&mut self, endpoint: Endpoint) {
        log::info!("[ClusterRoomDataChannel {}/Subscribers] user leaves, clean up", self.room);
        if let Some(channels) = self.subscriptions.remove(&endpoint) {
            for c in channels {
                if let Some(channel) = self.channels.get_mut(&c) {
                    channel.subscribers.remove(&endpoint);
                    if channel.subscribers.is_empty() {
                        self.channels.remove(&c);
                        self.queue.push_back(Output::Pubsub(pubsub::Control(c, ChannelControl::UnsubAuto)));
                    }
                }
            }
        }
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for MessageChannelSubscriber<Endpoint> {
    type Time = ();

    fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.subscriptions.is_empty() && self.channels.is_empty()
    }

    fn empty_event(&self) -> Output<Endpoint> {
        Output::OnResourceEmpty
    }

    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint> Drop for MessageChannelSubscriber<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomDataChannel {}/Subscriber] Drop", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
        assert_eq!(self.subscriptions.len(), 0, "Subscribers not empty on drop");
        assert_eq!(self.channels.len(), 0, "Channels not not empty on drop");
    }
}
