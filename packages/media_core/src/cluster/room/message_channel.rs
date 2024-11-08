use atm0s_sdn::features::pubsub;
use media_server_protocol::message_channel::MessageChannelPacket;
use publisher::MessageChannelPublisher;
use sans_io_runtime::{TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild};
use std::{fmt::Debug, hash::Hash};
use subscriber::MessageChannelSubscriber;

use crate::{
    cluster::{ClusterEndpointEvent, ClusterRoomHash},
    endpoint::MessageChannelLabel,
};

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

pub struct RoomMessageChannel<Endpoint> {
    room: ClusterRoomHash,
    publisher: TaskSwitcherBranch<MessageChannelPublisher<Endpoint>, Output<Endpoint>>,
    subscriber: TaskSwitcherBranch<MessageChannelSubscriber<Endpoint>, Output<Endpoint>>,
    switcher: TaskSwitcher,
}

impl<Endpoint: Hash + Eq + Copy + Debug> RoomMessageChannel<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        log::info!("[ClusterRoomDataChannel {}] Create virtual datachannel", room);
        Self {
            room,
            publisher: TaskSwitcherBranch::new(MessageChannelPublisher::new(room), TaskType::Publisher),
            subscriber: TaskSwitcherBranch::new(MessageChannelSubscriber::new(room), TaskType::Subscriber),
            switcher: TaskSwitcher::new(2),
        }
    }

    pub fn on_pubsub_event(&mut self, event: pubsub::Event) {
        let channel_id = event.0;
        if let pubsub::ChannelEvent::SourceData(_, data) = event.1 {
            self.subscriber.input(&mut self.switcher).on_channel_data(channel_id, data);
        }
    }

    pub fn on_channel_publish_start(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        self.publisher.input(&mut self.switcher).on_channel_pub_start(endpoint, label);
    }

    pub fn on_channel_publish_stop(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        self.publisher.input(&mut self.switcher).on_channel_pub_stop(endpoint, label);
    }

    pub fn on_channel_data(&mut self, endpoint: Endpoint, label: &MessageChannelLabel, data: MessageChannelPacket) {
        self.publisher.input(&mut self.switcher).on_channel_data(endpoint, label, data);
    }

    pub fn on_channel_subscribe(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        self.subscriber.input(&mut self.switcher).on_channel_subscribe(endpoint, label);
    }

    pub fn on_channel_unsubscribe(&mut self, endpoint: Endpoint, label: &MessageChannelLabel) {
        self.subscriber.input(&mut self.switcher).on_channel_unsubscribe(endpoint, label);
    }

    pub fn on_leave(&mut self, endpoint: Endpoint) {
        self.subscriber.input(&mut self.switcher).on_leave(endpoint);
        self.publisher.input(&mut self.switcher).on_leave(endpoint);
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for RoomMessageChannel<Endpoint> {
    type Time = ();

    fn is_empty(&self) -> bool {
        self.publisher.is_empty() && self.subscriber.is_empty()
    }

    fn empty_event(&self) -> Output<Endpoint> {
        Output::OnResourceEmpty
    }

    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Publisher => {
                    if let Some(out) = self.publisher.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            // we dont need to forward OnResourceEmpty to parent
                        } else {
                            return Some(out);
                        }
                    }
                }
                TaskType::Subscriber => {
                    if let Some(out) = self.subscriber.pop_output((), &mut self.switcher) {
                        if let Output::OnResourceEmpty = out {
                            // we dont need to forward OnResourceEmpty to parent
                        } else {
                            return Some(out);
                        }
                    }
                }
            }
        }
    }
}

impl<Endpoint> Drop for RoomMessageChannel<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoomDataChannel] Drop {}", self.room);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use atm0s_sdn::features::pubsub::{self, ChannelControl};
    use media_server_protocol::{endpoint::PeerId, message_channel::MessageChannelPacket};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::{
        cluster::{
            id_generator,
            room::message_channel::{Output, RoomMessageChannel},
            ClusterEndpointEvent,
        },
        endpoint::MessageChannelLabel,
    };

    #[test]
    fn start_stop_publish() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomMessageChannel::new(room_id);
        let user1 = 1;
        let user2 = 2;
        let label1 = &MessageChannelLabel("test".to_string());

        // 1 -> test
        let channel_id1 = id_generator::gen_msg_channel_id(room_id, label1);

        room.on_channel_publish_start(user1, label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::PubStart))));
        assert_eq!(room.pop_output(now), None);

        room.on_channel_publish_start(user2, label1);
        assert_eq!(room.pop_output(now), None);

        room.on_channel_publish_stop(user1, label1);
        assert_eq!(room.pop_output(now), None);

        room.on_channel_publish_stop(user2, label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::PubStop))));
        assert_eq!(room.pop_output(now), None);
    }

    #[test]
    fn sub_unsub() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomMessageChannel::new(room_id);
        let user1 = 1;
        let user2 = 2;
        let user3 = 3;
        let label1 = MessageChannelLabel("test".to_string());
        let label2 = MessageChannelLabel("test2".to_string());

        // 1 -> test
        // 2 -> test
        // 3 -> test2
        let channel_id1 = id_generator::gen_msg_channel_id(room_id, &label1);
        let channel_id2 = id_generator::gen_msg_channel_id(room_id, &label2);

        room.on_channel_subscribe(user1, &label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::SubAuto))));
        assert_eq!(room.pop_output(now), None);

        // Second subscriber will do nothing but register in the subscriber list
        room.on_channel_subscribe(user2, &label1);
        assert_eq!(room.pop_output(now), None);

        // First subscriber of a new channel should start publish and subscribe too
        room.on_channel_subscribe(user3, &label2);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::SubAuto))));

        // Last subscriber that unsubscribes will stop the channel
        room.on_channel_unsubscribe(user1, &label1);
        assert_eq!(room.pop_output(now), None);
        room.on_channel_unsubscribe(user2, &label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);

        room.on_channel_unsubscribe(user3, &label2);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);

        assert!(room.subscriber.is_empty());
    }

    #[test]
    fn receive_data() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomMessageChannel::new(room_id);
        let user1 = 1;
        let user2 = 2;
        let label1 = MessageChannelLabel("test".to_string());

        let channel_id1 = id_generator::gen_msg_channel_id(room_id, &label1);

        room.on_channel_subscribe(user1, &label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::SubAuto))));

        room.on_channel_subscribe(user2, &label1);
        assert_eq!(room.pop_output(now), None);

        let peer_id = PeerId::from("testid");
        let pkt = MessageChannelPacket {
            from: peer_id.clone(),
            data: vec![1, 2, 3],
        };
        room.on_pubsub_event(pubsub::Event(channel_id1, pubsub::ChannelEvent::SourceData(1, pkt.serialize())));
        let mut receivers = HashMap::new();
        receivers.insert(user1, false);
        receivers.insert(user2, false);

        if let Some(out) = room.pop_output(now) {
            match out {
                Output::Endpoint(endpoints, ClusterEndpointEvent::MessageChannelData(label, peer, data)) => {
                    assert_eq!(label, label1);
                    assert_eq!(peer, peer_id);
                    assert_eq!(data, pkt.data);
                    for endpoint in endpoints {
                        *receivers.get_mut(&endpoint).unwrap() = true;
                    }
                }
                _ => panic!("Unexpected output: {:?}", out),
            }
        }
        assert!(receivers[&user1]);
        assert!(receivers[&user2]);
        assert_eq!(room.pop_output(now), None);

        room.on_channel_unsubscribe(user1, &label1);
        room.on_channel_unsubscribe(user2, &label1);

        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);
    }

    #[test]
    fn publish_data() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomMessageChannel::new(room_id);
        let user1 = 1;
        let user2 = 2;
        let label1 = &MessageChannelLabel("test".to_string());

        let channel_id1 = id_generator::gen_msg_channel_id(room_id, label1);

        room.on_channel_subscribe(user1, label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::SubAuto))));

        room.on_channel_subscribe(user2, label1);
        assert_eq!(room.pop_output(now), None);

        let peer_id = PeerId::from("testid");
        let pkt = MessageChannelPacket {
            from: peer_id.clone(),
            data: vec![1, 2, 3],
        };
        room.on_channel_publish_start(user1, label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::PubStart))));

        room.on_channel_data(user1, label1, pkt.clone());
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::PubData(pkt.serialize())))));

        room.on_channel_publish_stop(user1, label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::PubStop))));
        assert_eq!(room.pop_output(now), None);

        room.on_channel_unsubscribe(user1, label1);
        room.on_channel_unsubscribe(user2, label1);

        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);
    }

    #[test]
    fn leave_room() {
        let now = ();
        let room_id = 1.into();
        let mut room = RoomMessageChannel::new(room_id);
        let user1 = 1;
        let user2 = 2;
        let label1 = &MessageChannelLabel("test".to_string());
        let label2 = &MessageChannelLabel("test2".to_string());

        let channel_id1 = id_generator::gen_msg_channel_id(room_id, label1);
        let channel_id2 = id_generator::gen_msg_channel_id(room_id, label2);

        room.on_channel_subscribe(user1, label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::SubAuto))));
        room.on_channel_subscribe(user1, label2);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::SubAuto))));

        room.on_channel_publish_start(user1, label1);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::PubStart))));
        room.on_channel_publish_start(user1, label2);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::PubStart))));

        room.on_channel_subscribe(user2, label1);
        assert_eq!(room.pop_output(now), None);

        room.on_leave(user1);
        let mut channels_check = HashSet::new();
        if let Some(Output::Pubsub(pubsub::Control(channel_id, action))) = room.pop_output(now) {
            assert_eq!(action, ChannelControl::PubStop);
            channels_check.insert(channel_id);
        }
        if let Some(Output::Pubsub(pubsub::Control(channel_id, action))) = room.pop_output(now) {
            assert_eq!(action, ChannelControl::PubStop);
            channels_check.insert(channel_id);
        }
        assert!(channels_check.contains(&channel_id1));
        assert!(channels_check.contains(&channel_id2));
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id2, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);

        room.on_leave(user2);
        assert_eq!(room.pop_output(now), Some(Output::Pubsub(pubsub::Control(channel_id1, ChannelControl::UnsubAuto))));
        assert_eq!(room.pop_output(now), None);
    }
}
