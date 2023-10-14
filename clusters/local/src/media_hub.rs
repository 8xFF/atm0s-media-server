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

#[cfg(test)]
mod test {
    use super::*;
    use cluster::generate_cluster_track_uuid;
    use transport::{MediaPacket, MediaPacketExtensions};

    #[test]
    fn test_local_media_hub() {
        let mut media_hub = LocalMediaHub::default();
        let track_uuid = generate_cluster_track_uuid("room", "peer", "track");
        let (tx, rx) = async_std::channel::bounded(100);
        media_hub.subscribe(track_uuid, 1, tx);
        let pkt = MediaPacket {
            pt: 111,
            seq_no: 1,
            time: 1000,
            marker: true,
            ext_vals: MediaPacketExtensions {
                abs_send_time: None,
                transport_cc: None,
            },
            nackable: true,
            payload: vec![1, 2, 3],
        };
        media_hub.relay(track_uuid, pkt.clone());
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackMedia(track_uuid, pkt.clone())));
        media_hub.unsubscribe(track_uuid, 1);
        media_hub.relay(track_uuid, pkt.clone());
        assert!(rx.try_recv().is_err());
    }
}
