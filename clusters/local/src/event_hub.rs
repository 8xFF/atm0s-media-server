use std::collections::HashMap;

use async_std::channel::Sender;
use cluster::{ClusterEndpointIncomingEvent, ClusterTrackMeta};

use self::room::RoomHub;

mod peer;
mod room;

#[derive(Default)]
pub struct LocalEventHub {
    rooms: HashMap<String, RoomHub>,
}

impl LocalEventHub {
    pub fn new() -> Self {
        Self { rooms: HashMap::new() }
    }

    pub fn add_track(&mut self, room: &str, peer_id: &str, track_name: &str, track_meta: ClusterTrackMeta) {
        let room_hub = self.rooms.entry(room.into()).or_insert_with(|| RoomHub::default());
        room_hub.add_track(peer_id, track_name, track_meta);
    }

    pub fn remove_track(&mut self, room: &str, peer_id: &str, track_name: &str) {
        if let Some(room_hub) = self.rooms.get_mut(room) {
            room_hub.remove_track(peer_id, track_name);
            if room_hub.is_empty() {
                self.rooms.remove(room);
            }
        }
    }

    pub fn subscribe_room(&mut self, room: &str, consumer_id: u32, tx: Sender<ClusterEndpointIncomingEvent>) {
        let room_hub = self.rooms.entry(room.into()).or_insert_with(|| RoomHub::default());
        room_hub.subscribe(consumer_id, tx);
    }

    pub fn unsubscribe_room(&mut self, room: &str, consumer_id: u32) {
        if let Some(room_hub) = self.rooms.get_mut(room) {
            room_hub.unsubscribe(consumer_id);
            if room_hub.is_empty() {
                self.rooms.remove(room);
            }
        }
    }

    pub fn subscribe_peer(&mut self, room: &str, peer: &str, consumer_id: u32, tx: Sender<ClusterEndpointIncomingEvent>) {
        let room_hub = self.rooms.entry(room.into()).or_insert_with(|| RoomHub::default());
        room_hub.subscribe_peer(peer, consumer_id, tx);
    }

    pub fn unsubscribe_peer(&mut self, room: &str, peer: &str, consumer_id: u32) {
        if let Some(room_hub) = self.rooms.get_mut(room) {
            room_hub.unsubscribe_peer(peer, consumer_id);
            if room_hub.is_empty() {
                self.rooms.remove(room);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cluster::{ClusterEndpointIncomingEvent, ClusterTrackMeta, ClusterTrackStatus};
    use transport::MediaKind;

    #[test]
    fn local_hub_sub_pre() {
        let mut local_hub = super::LocalEventHub::default();

        let (tx, rx) = async_std::channel::bounded(10);
        local_hub.subscribe_room("room1", 100, tx);

        let meta = ClusterTrackMeta {
            kind: MediaKind::Audio,
            active: true,
            label: None,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            scaling: "Single".to_string(),
        };
        local_hub.add_track("room1", "peer1", "track1", meta.clone());

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta.clone())));

        local_hub.remove_track("room1", "peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));

        local_hub.unsubscribe_room("room1", 100);

        local_hub.add_track("room1", "peer1", "track2", meta.clone());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn local_hub_sub_after() {
        let mut local_hub = super::LocalEventHub::default();

        let (tx, rx) = async_std::channel::bounded(10);

        let meta = ClusterTrackMeta {
            kind: MediaKind::Audio,
            active: true,
            label: None,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            scaling: "Single".to_string(),
        };
        local_hub.add_track("room1", "peer1", "track1", meta.clone());
        local_hub.subscribe_room("room1", 100, tx);

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta)));
        local_hub.remove_track("room1", "peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));
    }

    #[test]
    fn local_hub_sub_peer_pre() {
        let mut local_hub = super::LocalEventHub::default();

        let (tx, rx) = async_std::channel::bounded(10);
        local_hub.subscribe_peer("room1", "peer1", 100, tx);

        let meta = ClusterTrackMeta {
            kind: MediaKind::Audio,
            active: true,
            label: None,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            scaling: "Single".to_string(),
        };
        local_hub.add_track("room1", "peer1", "track1", meta.clone());
        local_hub.add_track("room1", "peer2", "track1", meta.clone());

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta.clone())));
        assert!(rx.try_recv().is_err());

        local_hub.remove_track("room1", "peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));

        local_hub.unsubscribe_peer("room1", "peer1", 100);

        local_hub.add_track("room1", "peer1", "track2", meta.clone());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn local_hub_sub_peer_after() {
        let mut local_hub = super::RoomHub::default();

        let (tx, rx) = async_std::channel::bounded(10);

        let meta = ClusterTrackMeta {
            kind: MediaKind::Audio,
            active: true,
            label: None,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            scaling: "Single".to_string(),
        };
        local_hub.add_track("peer1", "track1", meta.clone());
        local_hub.add_track("peer2", "track1", meta.clone());
        local_hub.subscribe_peer("peer1", 100, tx);

        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), meta)));
        assert!(rx.try_recv().is_err());

        local_hub.remove_track("peer1", "track1");
        assert_eq!(rx.try_recv(), Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string())));
    }
}
