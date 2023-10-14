use std::collections::HashMap;

use async_std::channel::Sender;
use cluster::{ClusterEndpointIncomingEvent, ClusterTrackMeta};

pub struct PeerHub {
    peer_id: String,
    tracks: HashMap<String, ClusterTrackMeta>,
    consumers: HashMap<u32, Sender<ClusterEndpointIncomingEvent>>,
}

impl PeerHub {
    pub fn new(peer_id: &str) -> Self {
        Self {
            peer_id: peer_id.into(),
            tracks: HashMap::new(),
            consumers: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.consumers.is_empty() && self.tracks.is_empty()
    }

    pub fn add_track(&mut self, track_name: &str, track_meta: ClusterTrackMeta) {
        self.tracks.insert(track_name.into(), track_meta.clone());
        let event = ClusterEndpointIncomingEvent::PeerTrackAdded(self.peer_id.clone(), track_name.into(), track_meta);
        self.fire_event(event);
    }

    pub fn remove_track(&mut self, track_name: &str) {
        self.tracks.remove(track_name);
        let event = ClusterEndpointIncomingEvent::PeerTrackRemoved(self.peer_id.clone(), track_name.into());
        self.fire_event(event);
    }

    pub fn subscribe(&mut self, consumer_id: u32, tx: Sender<ClusterEndpointIncomingEvent>) {
        self.fire_to(&tx);
        self.consumers.insert(consumer_id, tx);
    }

    pub fn unsubscribe(&mut self, consumer_id: u32) {
        self.consumers.remove(&consumer_id);
    }

    fn fire_event(&self, event: ClusterEndpointIncomingEvent) {
        for (_, tx) in self.consumers.iter() {
            if let Err(_e) = tx.try_send(event.clone()) {
                todo!("handle this")
            }
        }
    }

    pub(crate) fn fire_to(&self, tx: &Sender<ClusterEndpointIncomingEvent>) {
        for (track_name, track_meta) in self.tracks.iter() {
            let event = ClusterEndpointIncomingEvent::PeerTrackAdded(self.peer_id.clone(), track_name.clone(), track_meta.clone());
            if let Err(_e) = tx.try_send(event) {
                todo!("handle this")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cluster::{ClusterEndpointIncomingEvent, ClusterTrackMeta, ClusterTrackStatus};
    use transport::MediaKind;

    use super::PeerHub;

    #[test]
    fn peer_hub_sub_pre() {
        let mut peer_hub = PeerHub::new("peer1");

        let (tx, rx) = async_std::channel::bounded(10);
        peer_hub.subscribe(100, tx);

        let meta = ClusterTrackMeta {
            kind: MediaKind::Audio,
            active: true,
            label: None,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            scaling: "Single".to_string(),
        };
        peer_hub.add_track("track1", meta.clone());

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta.clone())));

        peer_hub.remove_track("track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));

        peer_hub.unsubscribe(100);
        peer_hub.add_track("track2", meta.clone());

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn peer_hub_sub_after() {
        let mut peer_hub = PeerHub::new("peer1");

        let (tx, rx) = async_std::channel::bounded(10);
        let meta = ClusterTrackMeta {
            kind: MediaKind::Audio,
            active: true,
            label: None,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            scaling: "Single".to_string(),
        };
        peer_hub.add_track("track1", meta.clone());
        peer_hub.subscribe(100, tx);

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta)));

        peer_hub.remove_track("track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));
    }
}
