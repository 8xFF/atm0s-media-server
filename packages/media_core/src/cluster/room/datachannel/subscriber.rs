use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId};
use media_server_protocol::datachannel::DataChannelPacket;
use sans_io_runtime::{return_if_none, TaskSwitcherChild};

use super::Output;
use crate::cluster::{ClusterEndpointEvent, ClusterRoomHash};

pub struct DataChannelSubscriber<Endpoint> {
    room: ClusterRoomHash,
    subscriptions: HashMap<Endpoint, HashSet<ChannelId>>,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Hash + Eq + Copy + Debug> DataChannelSubscriber<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            queue: VecDeque::new(),
            subscriptions: HashMap::new(),
        }
    }

    pub fn get_subscriptions(&self, endpoint: Endpoint) -> Vec<ChannelId> {
        self.subscriptions.get(&endpoint).map_or_else(Vec::new, |s| s.iter().copied().collect())
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.subscriptions.is_empty()
    }

    pub fn on_channel_create(&mut self, endpoint: Endpoint, channel_id: ChannelId) {
        self.subscriptions.entry(endpoint).or_insert(HashSet::new()).insert(channel_id);
        self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto)));
    }

    pub fn on_channel_close(&mut self, channel_id: ChannelId) {
        self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto)));
    }

    pub fn on_channel_subscribe(&mut self, endpoint: Endpoint, channel_id: ChannelId) {
        self.subscriptions.entry(endpoint).or_insert(HashSet::new()).insert(channel_id);
    }

    pub fn on_channel_unsubscribe(&mut self, endpoint: Endpoint, channel_id: ChannelId) {
        if let Some(subscription) = self.subscriptions.get_mut(&endpoint) {
            subscription.remove(&channel_id);
            if subscription.is_empty() {
                self.subscriptions.remove(&endpoint);
            }
        }
    }

    pub fn on_channel_data(&mut self, key: String, endpoints: Vec<Endpoint>, data: Vec<u8>) {
        let pkt = return_if_none!(DataChannelPacket::deserialize(&data));
        self.queue
            .push_back(Output::Endpoint(endpoints, ClusterEndpointEvent::VirtualChannelMessage(key, pkt.from.clone(), pkt.data.clone())));
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
        assert_eq!(self.subscriptions.len(), 0, "Subscribers not empty on drop");
    }
}
