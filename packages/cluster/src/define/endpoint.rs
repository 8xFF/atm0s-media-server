use transport::TrackId;

use crate::{
    ClusterEndpointError, ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent, ClusterPeerId, ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent, ClusterTrackMeta,
    ClusterTrackName, ClusterTrackUuid,
};

#[async_trait::async_trait]
pub trait ClusterEndpoint: Send + Sync {
    fn on_event(&mut self, event: ClusterEndpointOutgoingEvent) -> Result<(), ClusterEndpointError>;
    async fn recv(&mut self) -> Result<ClusterEndpointIncomingEvent, ClusterEndpointError>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClusterEndpointOutgoingEvent {
    SubscribeRoom,
    UnsubscribeRoom,
    SubscribePeer(ClusterPeerId),
    UnsubscribePeer(ClusterPeerId),
    LocalTrackEvent(TrackId, ClusterLocalTrackOutgoingEvent),
    RemoteTrackEvent(TrackId, ClusterTrackUuid, ClusterRemoteTrackOutgoingEvent),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClusterEndpointIncomingEvent {
    PeerTrackAdded(ClusterPeerId, ClusterTrackName, ClusterTrackMeta),
    PeerTrackUpdated(ClusterPeerId, ClusterTrackName, ClusterTrackMeta),
    PeerTrackRemoved(ClusterPeerId, ClusterTrackName),
    LocalTrackEvent(TrackId, ClusterLocalTrackIncomingEvent),
    RemoteTrackEvent(TrackId, ClusterRemoteTrackIncomingEvent),
}
