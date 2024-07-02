use std::{collections::VecDeque, fmt::Debug, hash::Hash};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId};
use media_server_protocol::datachannel::DataChannelPacket;
use sans_io_runtime::TaskSwitcherChild;

use crate::cluster::{id_generator, ClusterRoomHash};

use super::Output;

pub struct DataChannelPublisher<Endpoint> {
    room: ClusterRoomHash,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Hash + Eq + Copy + Debug> DataChannelPublisher<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self { room, queue: VecDeque::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn on_channel_data(&mut self, key: &str, data: DataChannelPacket) {
        log::trace!("[ClusterRoomDataChannel {}/Publishers] publish virtual datachannel", self.room);
        let data = data.serialize();
        let channel_id: ChannelId = id_generator::gen_datachannel_id(self.room, key.to_string());
        self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubData(data))))
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for DataChannelPublisher<Endpoint> {
    type Time = ();
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint> Drop for DataChannelPublisher<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomDataChannel {}/Publishers] Drop", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
    }
}
