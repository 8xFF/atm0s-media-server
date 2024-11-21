use std::{collections::VecDeque, fmt::Debug, hash::Hash};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId};
use indexmap::{map::Entry, IndexMap, IndexSet};
use media_server_protocol::message_channel::MessageChannelPacket;
use media_server_utils::Count;
use sans_io_runtime::{return_if_none, TaskSwitcherChild};

use crate::{
    cluster::{id_generator, ClusterRoomHash},
    endpoint::MessageChannelLabel,
};

use super::Output;

#[derive(Debug)]
struct ChannelContainer<Endpoint: Debug> {
    publishers: IndexSet<Endpoint>,
}

#[derive(Debug)]
pub struct MessageChannelPublisher<Endpoint: Debug> {
    _c: Count<Self>,
    room: ClusterRoomHash,
    channels: IndexMap<ChannelId, ChannelContainer<Endpoint>>,
    publishers: IndexMap<Endpoint, IndexSet<ChannelId>>,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Debug + Hash + Eq + Copy> MessageChannelPublisher<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            _c: Default::default(),
            room,
            queue: VecDeque::new(),
            channels: IndexMap::new(),
            publishers: IndexMap::new(),
        }
    }

    pub fn on_channel_pub_start(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        log::info!("[ClusterRoomDataChannel {}/Publishers] publish start message channel", self.room);

        let channel_id: ChannelId = id_generator::gen_msg_channel_id(self.room, label);

        match self.channels.entry(channel_id) {
            Entry::Occupied(mut o) => {
                o.get_mut().publishers.insert(endpoint);
            }
            Entry::Vacant(v) => {
                let mut channel = ChannelContainer { publishers: IndexSet::new() };
                channel.publishers.insert(endpoint);
                v.insert(channel);
                self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart)));
            }
        }

        self.publishers.entry(endpoint).or_default().insert(channel_id);
    }

    pub fn on_channel_pub_stop(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        log::info!("[ClusterRoomDataChannel {}/Publishers] publish start message channel", self.room);

        let channel_id: ChannelId = id_generator::gen_msg_channel_id(self.room, label);
        let channel = return_if_none!(self.channels.get_mut(&channel_id));

        channel.publishers.swap_remove(&endpoint);

        if let Some(publisher) = self.publishers.get_mut(&endpoint) {
            publisher.swap_remove(&channel_id);
            if publisher.is_empty() {
                self.publishers.swap_remove(&endpoint);
            }
        }

        if channel.publishers.is_empty() {
            self.channels.swap_remove(&channel_id);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop)));
        }
    }

    pub fn on_leave(&mut self, endpoint: Endpoint) {
        log::info!("[ClusterRoomDataChannel {}/Publishers] user leaves, clean up", self.room);
        if let Some(channels) = self.publishers.swap_remove(&endpoint) {
            for c in channels {
                if let Some(channel) = self.channels.get_mut(&c) {
                    channel.publishers.swap_remove(&endpoint);
                    if channel.publishers.is_empty() {
                        self.channels.swap_remove(&c);
                        self.queue.push_back(Output::Pubsub(pubsub::Control(c, ChannelControl::PubStop)));
                    }
                }
            }
        }
    }

    pub fn on_channel_data(&mut self, endpoint: Endpoint, label: &MessageChannelLabel, data: MessageChannelPacket) {
        log::info!("[ClusterRoomDataChannel {}/Publishers] publish to message datachannel", self.room);

        let channel_id: ChannelId = id_generator::gen_msg_channel_id(self.room, label);
        let channel = return_if_none!(self.channels.get(&channel_id));
        if channel.publishers.contains(&endpoint) {
            let data = data.serialize();
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubData(data))))
        } else {
            log::warn!("[ClusterRoomMessageChannel {}/Publisher] publish without start", self.room);
        }
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for MessageChannelPublisher<Endpoint> {
    type Time = ();

    fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.channels.is_empty() && self.publishers.is_empty()
    }

    fn empty_event(&self) -> Output<Endpoint> {
        Output::OnResourceEmpty
    }

    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint: Debug> Drop for MessageChannelPublisher<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomDataChannel {}/Publishers] Drop", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop {:?}", self.queue);
        assert_eq!(self.publishers.len(), 0, "Publishers not empty on drop {:?}", self.publishers);
        assert_eq!(self.channels.len(), 0, "Channels not empty on drop {:?}", self.channels);
    }
}
