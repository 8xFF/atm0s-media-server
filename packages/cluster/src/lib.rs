use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use transport::{MediaKind, MediaPacket};

pub type ClusterTrackUuid = u64;
pub type ClusterPeerId = String;
pub type ClusterTrackName = String;
pub type ClusterConsumerId = u64;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
pub enum ClusterTrackStatus {
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "reconnecting")]
    Reconnecting,
    #[serde(rename = "disconnected")]
    Disconnected,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ClusterTrackMeta {
    pub kind: MediaKind,
    pub scaling: String,
    pub layers: Vec<u32>,
    pub status: ClusterTrackStatus,
    pub active: bool,
    pub label: Option<String>,
}

pub enum ClusterEndpointError {
    InternalError,
    NotImplement,
}

#[derive(Clone)]
pub enum ClusterEndpointIncomingEvent {
    PeerTrackMedia(ClusterTrackUuid, MediaPacket),
    PeerTrackAdded(ClusterPeerId, ClusterTrackName, ClusterTrackMeta),
    PeerTrackUpdated(ClusterPeerId, ClusterTrackName, ClusterTrackMeta),
    PeerTrackRemoved(ClusterPeerId, ClusterTrackName),
}

pub enum ClusterEndpointOutgoingEvent {
    TrackMedia(ClusterTrackUuid, MediaPacket),
    TrackAdded(ClusterTrackName, ClusterTrackMeta),
    TrackRemoved(ClusterTrackName),
    SubscribeRoom,
    UnsubscribeRoom,
    SubscribePeer(ClusterPeerId),
    UnsubscribePeer(ClusterPeerId),
    SubscribeTrack(ClusterPeerId, ClusterTrackName, ClusterConsumerId),
    UnsubscribeTrack(ClusterPeerId, ClusterTrackName, ClusterConsumerId),
}

/// generate for other peer
pub fn generate_cluster_track_uuid(room_id: &str, peer_id: &str, track_name: &str) -> ClusterTrackUuid {
    let based = format!("{}-{}-{}", room_id, peer_id, track_name);
    let mut s = DefaultHasher::new();
    based.hash(&mut s);
    s.finish()
}

#[async_trait::async_trait]
pub trait ClusterEndpoint: Send + Sync {
    fn on_event(&mut self, event: ClusterEndpointOutgoingEvent) -> Result<(), ClusterEndpointError>;
    async fn recv(&mut self) -> Result<ClusterEndpointIncomingEvent, ClusterEndpointError>;
}

#[async_trait::async_trait]
pub trait Cluster<C>: Send + Sync
where
    C: ClusterEndpoint,
{
    fn build(&mut self, room_id: &str, peer_id: &str) -> C;
}
