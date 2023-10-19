use std::collections::HashMap;

use async_std::channel::Sender;
use cluster::{ClusterEndpointIncomingEvent, ClusterTrackMeta};

use super::peer::PeerHub;

#[derive(Default)]
pub struct RoomHub {
    consumers: HashMap<u32, Sender<ClusterEndpointIncomingEvent>>,
    peers: HashMap<String, PeerHub>,
}

impl RoomHub {
    pub fn is_empty(&self) -> bool {
        self.consumers.is_empty() && self.peers.is_empty()
    }

    pub fn add_track(&mut self, peer_id: &str, track_name: &str, track_meta: ClusterTrackMeta) {
        self.fire_event(ClusterEndpointIncomingEvent::PeerTrackAdded(peer_id.into(), track_name.into(), track_meta.clone()));
        let peer_hub = self.peers.entry(peer_id.into()).or_insert_with(|| PeerHub::new(peer_id));
        peer_hub.add_track(track_name, track_meta);
    }

    pub fn remove_track(&mut self, peer_id: &str, track_name: &str) {
        self.fire_event(ClusterEndpointIncomingEvent::PeerTrackRemoved(peer_id.into(), track_name.into()));
        if let Some(peer_hub) = self.peers.get_mut(peer_id) {
            peer_hub.remove_track(track_name);
            if peer_hub.is_empty() {
                self.peers.remove(peer_id);
            }
        }
    }

    pub fn subscribe(&mut self, consumer_id: u32, tx: Sender<ClusterEndpointIncomingEvent>) {
        self.fire_to(&tx);
        self.consumers.insert(consumer_id, tx);
    }

    pub fn unsubscribe(&mut self, consumer_id: u32) {
        self.consumers.remove(&consumer_id);
    }

    pub fn subscribe_peer(&mut self, peer: &str, consumer_id: u32, tx: Sender<ClusterEndpointIncomingEvent>) {
        let peer_hub = self.peers.entry(peer.into()).or_insert_with(|| PeerHub::new(peer));
        peer_hub.subscribe(consumer_id, tx);
    }

    pub fn unsubscribe_peer(&mut self, peer: &str, consumer_id: u32) {
        if let Some(peer_hub) = self.peers.get_mut(peer) {
            peer_hub.unsubscribe(consumer_id);
            if peer_hub.is_empty() {
                self.peers.remove(peer);
            }
        }
    }

    fn fire_event(&self, event: ClusterEndpointIncomingEvent) {
        for (_, tx) in self.consumers.iter() {
            if let Err(_e) = tx.try_send(event.clone()) {
                todo!("handle this")
            }
        }
    }

    fn fire_to(&self, tx: &Sender<ClusterEndpointIncomingEvent>) {
        for (_, peer) in self.peers.iter() {
            peer.fire_to(tx);
        }
    }
}

#[cfg(test)]
mod tests {
    use cluster::{ClusterEndpointIncomingEvent, ClusterTrackMeta, ClusterTrackStatus};
    use transport::MediaKind;

    #[test]
    fn room_hub_sub_pre() {
        let mut room_hub = super::RoomHub::default();

        let (tx, rx) = async_std::channel::bounded(10);
        room_hub.subscribe(100, tx);

        let meta = ClusterTrackMeta::default_audio();
        room_hub.add_track("peer1", "track1", meta.clone());

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta.clone())));

        room_hub.remove_track("peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));

        room_hub.unsubscribe(100);

        room_hub.add_track("peer1", "track2", meta.clone());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn room_hub_sub_after() {
        let mut room_hub = super::RoomHub::default();

        let (tx, rx) = async_std::channel::bounded(10);

        let meta = ClusterTrackMeta::default_audio();
        room_hub.add_track("peer1", "track1", meta.clone());
        room_hub.subscribe(100, tx);

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta)));
        room_hub.remove_track("peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));
    }

    #[test]
    fn room_hub_sub_peer_pre() {
        let mut room_hub = super::RoomHub::default();

        let (tx, rx) = async_std::channel::bounded(10);
        room_hub.subscribe_peer("peer1", 100, tx);

        let meta = ClusterTrackMeta::default_audio();
        room_hub.add_track("peer1", "track1", meta.clone());
        room_hub.add_track("peer2", "track1", meta.clone());

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta.clone())));
        assert!(rx.try_recv().is_err());

        room_hub.remove_track("peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));

        room_hub.unsubscribe_peer("peer1", 100);

        room_hub.add_track("peer1", "track2", meta.clone());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn room_hub_sub_peer_after() {
        let mut room_hub = super::RoomHub::default();

        let (tx, rx) = async_std::channel::bounded(10);

        let meta = ClusterTrackMeta::default_audio();
        room_hub.add_track("peer1", "track1", meta.clone());
        room_hub.add_track("peer2", "track1", meta.clone());
        room_hub.subscribe_peer("peer1", 100, tx);

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta)));
        assert!(rx.try_recv().is_err());

        room_hub.remove_track("peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));
    }
}
