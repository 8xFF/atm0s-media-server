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
    subscribers: Vec<Endpoint>,
    key: String,
}

pub struct DataChannelSubscriber<Endpoint> {
    room: ClusterRoomHash,
    channels: HashMap<ChannelId, ChannelContainer<Endpoint>>,
    // subscribers: HashMap<Endpoint, (ChannelId, PeerId)>,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Hash + Eq + Copy + Debug> DataChannelSubscriber<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            queue: VecDeque::new(),
            channels: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.channels.is_empty()
    }

    pub fn on_channel_subscribe(&mut self, endpoint: Endpoint, key: &str) {
        let channel_id: ChannelId = id_generator::gen_datachannel_id(self.room, endpoint, key.to_string());
        log::info!("[ClusterRoom {}/Subscribers] peer {:?} subscribe channel: {channel_id}", self.room, endpoint);
        let channel_container = self.channels.entry(channel_id).or_insert(ChannelContainer {
            subscribers: vec![],
            key: key.to_string(),
        });
        channel_container.subscribers.push(endpoint);
        if channel_container.subscribers.len() == 1 {
            log::info!("[ClusterRoom {}/Subscribers] first subscriber => Sub channel {channel_id}", self.room);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto)));
        }
    }

    pub fn on_channel_unsubscribe(&mut self, endpoint: Endpoint, key: &str) {
        let channel_id: ChannelId = id_generator::gen_datachannel_id(self.room, endpoint, key.to_string());
        log::info!("[ClusterRoom {}/Subscribers] peer {:?} unsubscribe channel: {channel_id}", self.room, endpoint);
        if let Some(channel_container) = self.channels.get_mut(&channel_id) {
            if let Some(index) = channel_container.subscribers.iter().position(|x| *x == endpoint) {
                channel_container.subscribers.swap_remove(index);
                if channel_container.subscribers.is_empty() {
                    log::info!("[ClusterRoom {}/Subscribers] last subscriber => Unsub channel {channel_id}", self.room);
                    self.channels.remove(&channel_id);
                    self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto)));
                }
            } else {
                log::warn!("[ClusterRoom {}/Subscribers] peer {:?} not subscribe in channel {channel_id}", self.room, endpoint);
            }
        }
    }

    pub fn on_channel_data(&mut self, channel_id: ChannelId, data: Vec<u8>) {
        log::info!("[ClusterRoom {}/Subscribers] Receive data from channel {channel_id}", self.room);
        let pkt = return_if_none!(DataChannelPacket::deserialize(&data));
        if let Some(channel_container) = self.channels.get(&channel_id) {
            for endpoint in &channel_container.subscribers {
                self.queue.push_back(Output::Endpoint(
                    vec![*endpoint],
                    ClusterEndpointEvent::ChannelMessage(channel_container.key.clone(), pkt.from.clone(), pkt.data.clone()),
                ));
            }
        } else {
            log::warn!("[ClusterRoom {}/Subscribers] Receive data from unknown channel {channel_id}", self.room);
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
        log::info!("[ClusterRoom {}/Subscriber] Drop", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
        assert_eq!(self.channels.len(), 0, "Channels not empty on drop");
    }
}
