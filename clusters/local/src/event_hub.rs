use std::collections::HashMap;

use async_std::channel::Sender;
use cluster::{ClusterEndpointIncomingEvent, ClusterTrackMeta};

struct PeerHub {
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
            if let Err(e) = tx.try_send(event.clone()) {
                todo!("handle this")
            }
        }
    }

    fn fire_to(&self, tx: &Sender<ClusterEndpointIncomingEvent>) {
        for (track_name, track_meta) in self.tracks.iter() {
            let event = ClusterEndpointIncomingEvent::PeerTrackAdded(self.peer_id.clone(), track_name.clone(), track_meta.clone());
            if let Err(e) = tx.try_send(event) {
                todo!("handle this")
            }
        }
    }
}

#[derive(Default)]
struct RoomHub {
    consumers: HashMap<u32, Sender<ClusterEndpointIncomingEvent>>,
    peers: HashMap<String, PeerHub>,
}

impl RoomHub {
    pub fn is_empty(&self) -> bool {
        self.consumers.is_empty() && self.peers.is_empty()
    }

    pub fn add_track(&mut self, peer_id: &str, track_name: &str, track_meta: ClusterTrackMeta) {
        let peer_hub = self.peers.entry(peer_id.into()).or_insert_with(|| PeerHub::new(peer_id));
        peer_hub.add_track(track_name, track_meta);
    }

    pub fn remove_track(&mut self, peer_id: &str, track_name: &str) {
        if let Some(peer_hub) = self.peers.get_mut(peer_id) {
            peer_hub.remove_track(track_name);
            if peer_hub.is_empty() {
                self.peers.remove(peer_id);
            }
        }
    }

    pub fn subscribe(&mut self, consumer_id: u32, tx: Sender<ClusterEndpointIncomingEvent>) {
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
}

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
