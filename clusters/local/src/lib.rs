use std::sync::Arc;

use async_std::channel::{Receiver, Sender};
use cluster::{generate_cluster_track_uuid, Cluster, ClusterEndpoint, ClusterEndpointError, ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent};
use event_hub::LocalEventHub;
use media_hub::LocalMediaHub;
use parking_lot::RwLock;

mod event_hub;
mod media_hub;

pub struct RoomLocal {
    consumer_id: u32,
    room_id: String,
    peer_id: String,
    event_hub: Arc<RwLock<LocalEventHub>>,
    media_hub: Arc<RwLock<LocalMediaHub>>,
    tx: Sender<ClusterEndpointIncomingEvent>,
    rx: Receiver<ClusterEndpointIncomingEvent>,
}

impl RoomLocal {
    pub fn new(consumer_id: u32, event_hub: Arc<RwLock<LocalEventHub>>, media_hub: Arc<RwLock<LocalMediaHub>>, room_id: &str, peer_id: &str) -> Self {
        let (tx, rx) = async_std::channel::bounded(100);
        Self {
            consumer_id,
            event_hub,
            media_hub,
            room_id: room_id.into(),
            peer_id: peer_id.into(),
            tx,
            rx,
        }
    }
}

#[async_trait::async_trait]
impl ClusterEndpoint for RoomLocal {
    fn on_event(&mut self, event: ClusterEndpointOutgoingEvent) -> Result<(), ClusterEndpointError> {
        match event {
            ClusterEndpointOutgoingEvent::TrackAdded(track_name, track_meta) => {
                self.event_hub.write().add_track(&self.room_id, &self.peer_id, &track_name, track_meta);
            }
            ClusterEndpointOutgoingEvent::TrackMedia(track_uuid, pkt) => {
                self.media_hub.read().relay(track_uuid, pkt);
            }
            ClusterEndpointOutgoingEvent::TrackRemoved(track_name) => {
                self.event_hub.write().remove_track(&self.room_id, &self.peer_id, &track_name);
            }
            ClusterEndpointOutgoingEvent::SubscribeTrack(peer_id, track_name, consumer_id) => {
                let track_uuid = generate_cluster_track_uuid(&self.room_id, &peer_id, &track_name);
                self.media_hub.write().subscribe(track_uuid, consumer_id, self.tx.clone());
            }
            ClusterEndpointOutgoingEvent::UnsubscribeTrack(peer_id, track_name, consumer_id) => {
                let track_uuid = generate_cluster_track_uuid(&self.room_id, &peer_id, &track_name);
                self.media_hub.write().unsubscribe(track_uuid, consumer_id);
            }
            ClusterEndpointOutgoingEvent::SubscribeRoom => {
                self.event_hub.write().subscribe_room(&self.room_id, self.consumer_id, self.tx.clone());
            }
            ClusterEndpointOutgoingEvent::UnsubscribeRoom => {
                self.event_hub.write().unsubscribe_room(&self.room_id, self.consumer_id);
            }
            ClusterEndpointOutgoingEvent::SubscribePeer(peer_id) => {
                self.event_hub.write().subscribe_peer(&self.room_id, &peer_id, self.consumer_id, self.tx.clone());
            }
            ClusterEndpointOutgoingEvent::UnsubscribePeer(peer_id) => {
                self.event_hub.write().unsubscribe_peer(&self.room_id, &peer_id, self.consumer_id);
            }
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<ClusterEndpointIncomingEvent, ClusterEndpointError> {
        self.rx.recv().await.map_err(|_| ClusterEndpointError::InternalError)
    }
}

pub struct ServerLocal {
    consumer_id_seed: u32,
    event_hub: Arc<RwLock<LocalEventHub>>,
    media_hub: Arc<RwLock<LocalMediaHub>>,
}

impl ServerLocal {
    pub fn new() -> Self {
        Self {
            consumer_id_seed: 0,
            event_hub: Arc::new(RwLock::new(LocalEventHub::default())),
            media_hub: Arc::new(RwLock::new(LocalMediaHub::default())),
        }
    }
}

impl Cluster<RoomLocal> for ServerLocal {
    fn build(&mut self, room_id: &str, peer_id: &str) -> RoomLocal {
        let consumer_id = self.consumer_id_seed;
        self.consumer_id_seed += 1;
        RoomLocal::new(consumer_id, self.event_hub.clone(), self.media_hub.clone(), room_id, peer_id)
    }
}
