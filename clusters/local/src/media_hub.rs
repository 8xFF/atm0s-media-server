use std::collections::HashMap;

use async_std::channel::Sender;
use cluster::{ClusterEndpointIncomingEvent, ClusterTrackUuid};
use transport::MediaPacket;

pub type ConsumerId = u64;

pub struct LocalChannel {
    consumers: HashMap<ConsumerId, Sender<ClusterEndpointIncomingEvent>>,
}

#[derive(Default)]
pub struct LocalMediaHub {
    channels: HashMap<ClusterTrackUuid, LocalChannel>,
}

impl LocalMediaHub {
    pub fn relay(&self, track_uuid: ClusterTrackUuid, pkt: MediaPacket) {
        if let Some(channel) = self.channels.get(&track_uuid) {
            for (_, tx) in channel.consumers.iter() {
                if let Err(e) = tx.try_send(ClusterEndpointIncomingEvent::PeerTrackMedia(track_uuid, pkt.clone())) {
                    todo!("handle this")
                }
            }
        }
    }

    pub fn subscribe(&mut self, track_uuid: ClusterTrackUuid, consumer_id: ConsumerId, tx: Sender<ClusterEndpointIncomingEvent>) {
        let channel = self.channels.entry(track_uuid).or_insert_with(|| LocalChannel { consumers: HashMap::new() });
        channel.consumers.insert(consumer_id, tx);
    }

    pub fn unsubscribe(&mut self, track_uuid: ClusterTrackUuid, consumer_id: ConsumerId) {
        if let Some(channel) = self.channels.get_mut(&track_uuid) {
            channel.consumers.remove(&consumer_id);
            if channel.consumers.is_empty() {
                self.channels.remove(&track_uuid);
            }
        }
    }
}
