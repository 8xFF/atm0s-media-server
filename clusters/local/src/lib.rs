use std::sync::Arc;

use async_std::channel::{Receiver, Sender};
use cluster::{
    generate_cluster_track_uuid, Cluster, ClusterEndpoint, ClusterEndpointError, ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackOutgoingEvent,
    ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent,
};
use event_hub::LocalEventHub;
use media_hub::LocalMediaHub;
use parking_lot::RwLock;
use utils::{hash_str, ResourceTracking};

mod event_hub;
mod media_hub;

pub struct PeerLocal {
    peer_id_hash: u64,
    room_id: String,
    peer_id: String,
    event_hub: Arc<RwLock<LocalEventHub>>,
    media_hub: Arc<RwLock<LocalMediaHub>>,
    tx: Sender<ClusterEndpointIncomingEvent>,
    rx: Receiver<ClusterEndpointIncomingEvent>,
    tracking: ResourceTracking,
}

impl PeerLocal {
    pub fn new(event_hub: Arc<RwLock<LocalEventHub>>, media_hub: Arc<RwLock<LocalMediaHub>>, room_id: &str, peer_id: &str) -> Self {
        let (tx, rx) = async_std::channel::bounded(100);
        log::debug!("[PeerLocal {}/{}] created", room_id, peer_id);
        Self {
            peer_id_hash: hash_str(&format!("{}-{}", room_id, peer_id)) << 16,
            event_hub,
            media_hub,
            room_id: room_id.into(),
            peer_id: peer_id.into(),
            tx,
            rx,
            tracking: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl ClusterEndpoint for PeerLocal {
    fn on_event(&mut self, event: ClusterEndpointOutgoingEvent) -> Result<(), ClusterEndpointError> {
        match event {
            ClusterEndpointOutgoingEvent::TrackAdded(track_id, track_name, track_meta) => {
                self.tracking.add2("track", &track_name);
                let track_uuid = generate_cluster_track_uuid(&self.room_id, &self.peer_id, &track_name);
                self.event_hub.write().add_track(&self.room_id, &self.peer_id, &track_name, track_meta);
                self.media_hub.write().add_track(track_uuid, track_id, self.tx.clone());
            }
            ClusterEndpointOutgoingEvent::TrackRemoved(_track_id, track_name) => {
                self.tracking.remove2("track", &track_name);
                let track_uuid = generate_cluster_track_uuid(&self.room_id, &self.peer_id, &track_name);
                self.event_hub.write().remove_track(&self.room_id, &self.peer_id, &track_name);
                self.media_hub.write().remove_track(track_uuid);
            }
            ClusterEndpointOutgoingEvent::SubscribeRoom => {
                self.tracking.add("sub-room");
                self.event_hub.write().subscribe_room(&self.room_id, self.peer_id_hash as u32, self.tx.clone());
            }
            ClusterEndpointOutgoingEvent::UnsubscribeRoom => {
                self.tracking.remove("sub-room");
                self.event_hub.write().unsubscribe_room(&self.room_id, self.peer_id_hash as u32);
            }
            ClusterEndpointOutgoingEvent::SubscribePeer(peer_id) => {
                self.tracking.add2("sub-peer", &peer_id);
                self.event_hub.write().subscribe_peer(&self.room_id, &peer_id, self.peer_id_hash as u32, self.tx.clone());
            }
            ClusterEndpointOutgoingEvent::UnsubscribePeer(peer_id) => {
                self.tracking.remove2("sub-peer", &peer_id);
                self.event_hub.write().unsubscribe_peer(&self.room_id, &peer_id, self.peer_id_hash as u32);
            }
            ClusterEndpointOutgoingEvent::LocalTrackEvent(track_id, event) => match event {
                ClusterLocalTrackOutgoingEvent::Subscribe(peer_id, track_name) => {
                    self.tracking.add3("sub-track", &peer_id, &track_name);
                    let consumer_id = self.peer_id_hash | track_id as u64;
                    let track_uuid = generate_cluster_track_uuid(&self.room_id, &peer_id, &track_name);
                    self.media_hub.write().subscribe(track_uuid, consumer_id, self.tx.clone());
                }
                ClusterLocalTrackOutgoingEvent::Unsubscribe(peer_id, track_name) => {
                    self.tracking.remove3("sub-track", &peer_id, &track_name);
                    let consumer_id = self.peer_id_hash | track_id as u64;
                    let track_uuid = generate_cluster_track_uuid(&self.room_id, &peer_id, &track_name);
                    self.media_hub.write().unsubscribe(track_uuid, consumer_id);
                }
                ClusterLocalTrackOutgoingEvent::RequestKeyFrame => {
                    let consumer_id = self.peer_id_hash | track_id as u64;
                    self.media_hub.read().forward(consumer_id, ClusterRemoteTrackIncomingEvent::RequestKeyFrame);
                }
            },
            ClusterEndpointOutgoingEvent::RemoteTrackEvent(_track_id, cluster_track_uuid, event) => match event {
                ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt) => {
                    self.media_hub.write().relay(cluster_track_uuid, pkt);
                }
            },
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<ClusterEndpointIncomingEvent, ClusterEndpointError> {
        self.rx.recv().await.map_err(|_| ClusterEndpointError::InternalError)
    }
}

impl Drop for PeerLocal {
    fn drop(&mut self) {
        log::debug!("[PeerLocal {}/{}] drop", self.room_id, self.peer_id);
        if !self.tracking.is_empty() {
            log::error!("PeerLocal {}-{} tracking not empty: {}", self.room_id, self.peer_id, self.tracking.dump());
        }
        assert!(self.tracking.is_empty());
    }
}

pub struct ServerLocal {
    event_hub: Arc<RwLock<LocalEventHub>>,
    media_hub: Arc<RwLock<LocalMediaHub>>,
}

impl ServerLocal {
    pub fn new() -> Self {
        Self {
            event_hub: Arc::new(RwLock::new(LocalEventHub::default())),
            media_hub: Arc::new(RwLock::new(LocalMediaHub::default())),
        }
    }
}

impl Cluster<PeerLocal> for ServerLocal {
    fn build(&mut self, room_id: &str, peer_id: &str) -> PeerLocal {
        PeerLocal::new(self.event_hub.clone(), self.media_hub.clone(), room_id, peer_id)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_std::prelude::FutureExt;
    use cluster::{Cluster, ClusterEndpoint, ClusterEndpointIncomingEvent, ClusterTrackMeta};

    #[async_std::test]
    async fn subscribe_room() {
        let mut server_local = super::ServerLocal::new();
        let mut peer1 = server_local.build("room", "peer1");
        let mut peer2 = server_local.build("room", "peer2");

        peer1.on_event(cluster::ClusterEndpointOutgoingEvent::SubscribeRoom).unwrap();
        let meta = ClusterTrackMeta {
            kind: transport::MediaKind::Audio,
            active: true,
            label: None,
            scaling: "Single".to_string(),
            layers: vec![],
            status: cluster::ClusterTrackStatus::Connected,
        };
        peer2.on_event(cluster::ClusterEndpointOutgoingEvent::TrackAdded(1, "audio_main".to_string(), meta.clone())).unwrap();

        assert_eq!(
            peer1.recv().timeout(Duration::from_secs(1)).await,
            Ok(Ok(ClusterEndpointIncomingEvent::PeerTrackAdded("peer2".to_string(), "audio_main".to_string(), meta.clone())))
        );

        peer2.on_event(cluster::ClusterEndpointOutgoingEvent::TrackRemoved(1, "audio_main".to_string())).unwrap();

        assert_eq!(
            peer1.recv().timeout(Duration::from_secs(1)).await,
            Ok(Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer2".to_string(), "audio_main".to_string())))
        );

        peer1.on_event(cluster::ClusterEndpointOutgoingEvent::UnsubscribeRoom).unwrap();
    }
}
