use std::collections::HashMap;

use async_std::channel::Sender;
use cluster::{ClusterEndpointIncomingEvent, ClusterLocalTrackIncomingEvent, ClusterRemoteTrackIncomingEvent, ClusterTrackStats, ClusterTrackUuid};
use transport::{MediaPacket, TrackId};

pub type ConsumerId = u64;

pub struct LocalChannel {
    track: Option<(TrackId, Sender<ClusterEndpointIncomingEvent>)>,
    consumers: HashMap<ConsumerId, Sender<ClusterEndpointIncomingEvent>>,
}

#[derive(Default)]
pub struct LocalMediaHub {
    channels: HashMap<ClusterTrackUuid, LocalChannel>,
}

impl LocalMediaHub {
    pub fn relay(&self, track_uuid: ClusterTrackUuid, pkt: MediaPacket) {
        if let Some(channel) = self.channels.get(&track_uuid) {
            for (consumer_id, tx) in channel.consumers.iter() {
                let local_track_id = (consumer_id & 0xffff) as u16;
                let event = ClusterEndpointIncomingEvent::LocalTrackEvent(local_track_id, ClusterLocalTrackIncomingEvent::MediaPacket(pkt.clone()));
                if let Err(_e) = tx.try_send(event) {
                    todo!("handle this")
                }
            }
        }
    }

    pub fn relay_stats(&self, track_uuid: ClusterTrackUuid, stats: ClusterTrackStats) {
        if let Some(channel) = self.channels.get(&track_uuid) {
            for (consumer_id, tx) in channel.consumers.iter() {
                let local_track_id = (consumer_id & 0xffff) as u16;
                let event = ClusterEndpointIncomingEvent::LocalTrackEvent(local_track_id, ClusterLocalTrackIncomingEvent::MediaStats(stats.clone()));
                if let Err(_e) = tx.try_send(event) {
                    todo!("handle this")
                }
            }
        }
    }

    pub fn forward(&self, consumer_id: ConsumerId, event: ClusterRemoteTrackIncomingEvent) {
        //TODO optimize this by create map beetween consumer_id and track_uuid
        for (_, channel) in &self.channels {
            if channel.consumers.contains_key(&consumer_id) {
                if let Some((track_id, tx)) = &channel.track {
                    let event = ClusterEndpointIncomingEvent::RemoteTrackEvent(*track_id, event.clone());
                    if let Err(_e) = tx.try_send(event) {
                        todo!("handle this")
                    }
                }
            }
        }
    }

    pub fn add_track(&mut self, track_uuid: ClusterTrackUuid, track_id: TrackId, tx: Sender<ClusterEndpointIncomingEvent>) {
        let channel = self.channels.entry(track_uuid).or_insert_with(|| LocalChannel {
            consumers: HashMap::new(),
            track: None,
        });
        channel.track = Some((track_id, tx));
    }

    pub fn remove_track(&mut self, track_uuid: ClusterTrackUuid) {
        if let Some(channel) = self.channels.get_mut(&track_uuid) {
            channel.track = None;
            if channel.consumers.is_empty() && channel.track.is_none() {
                self.channels.remove(&track_uuid);
            }
        }
    }

    pub fn subscribe(&mut self, track_uuid: ClusterTrackUuid, consumer_id: ConsumerId, tx: Sender<ClusterEndpointIncomingEvent>) {
        let channel = self.channels.entry(track_uuid).or_insert_with(|| LocalChannel {
            consumers: HashMap::new(),
            track: None,
        });
        channel.consumers.insert(consumer_id, tx);
    }

    pub fn unsubscribe(&mut self, track_uuid: ClusterTrackUuid, consumer_id: ConsumerId) {
        if let Some(channel) = self.channels.get_mut(&track_uuid) {
            channel.consumers.remove(&consumer_id);
            if channel.consumers.is_empty() && channel.track.is_none() {
                self.channels.remove(&track_uuid);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cluster::generate_cluster_track_uuid;
    use transport::{MediaPacket, RequestKeyframeKind};

    #[test]
    fn test_local_media_hub() {
        let mut media_hub = LocalMediaHub::default();
        let track_uuid = generate_cluster_track_uuid("room", "peer", "track");
        let (tx, rx) = async_std::channel::bounded(100);
        media_hub.subscribe(track_uuid, 1, tx);
        let pkt = MediaPacket::simple_audio(1, 1000, vec![1, 2, 3]);
        media_hub.relay(track_uuid, pkt.clone());
        assert_eq!(
            rx.try_recv(),
            Ok(ClusterEndpointIncomingEvent::LocalTrackEvent(1, ClusterLocalTrackIncomingEvent::MediaPacket(pkt.clone())))
        );
        media_hub.unsubscribe(track_uuid, 1);
        media_hub.relay(track_uuid, pkt.clone());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_local_media_hub_forward() {
        let mut media_hub = LocalMediaHub::default();
        let track_uuid = generate_cluster_track_uuid("room", "peer", "track");
        let track_id = 1;
        let consumer_id = 1001;

        let (tx, rx) = async_std::channel::bounded(100);
        media_hub.add_track(track_uuid, track_id, tx);

        let (tx2, _rx2) = async_std::channel::bounded(100);
        media_hub.subscribe(track_uuid, consumer_id, tx2);

        media_hub.forward(consumer_id, ClusterRemoteTrackIncomingEvent::RequestKeyFrame(RequestKeyframeKind::Pli));
        assert_eq!(
            rx.try_recv(),
            Ok(ClusterEndpointIncomingEvent::RemoteTrackEvent(
                track_id,
                ClusterRemoteTrackIncomingEvent::RequestKeyFrame(RequestKeyframeKind::Pli)
            ))
        );
        media_hub.remove_track(track_uuid);
        media_hub.forward(consumer_id, ClusterRemoteTrackIncomingEvent::RequestKeyFrame(RequestKeyframeKind::Pli));
        assert!(rx.try_recv().is_err());
    }
}
