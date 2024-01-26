use poem_openapi::Enum;
use protocol::media_event_logs::MediaEndpointLogRequest;
use serde::{Deserialize, Serialize};
use transport::TrackId;

use crate::{
    rpc::general::MediaSessionProtocol, ClusterEndpointError, ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent, ClusterPeerId, ClusterRemoteTrackIncomingEvent,
    ClusterRemoteTrackOutgoingEvent, ClusterTrackMeta, ClusterTrackName, ClusterTrackUuid,
};

#[async_trait::async_trait]
pub trait ClusterEndpoint: Send + Sync {
    fn on_event(&mut self, event: ClusterEndpointOutgoingEvent) -> Result<(), ClusterEndpointError>;
    async fn recv(&mut self) -> Result<ClusterEndpointIncomingEvent, ClusterEndpointError>;
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
pub enum ClusterStateEndpointState {
    New,
    Connecting,
    Connected,
    Reconnecting,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone, Enum)]
pub enum ClusterEndpointSubscribeScope {
    Full,
    #[serde(rename = "stream_only")]
    #[oai(rename = "stream_only")]
    StreamOnly,
    Manual,
}

impl ClusterEndpointSubscribeScope {
    pub fn is_sub_peers(&self) -> bool {
        matches!(self, ClusterEndpointSubscribeScope::Full)
    }

    pub fn is_sub_streams(&self) -> bool {
        matches!(self, ClusterEndpointSubscribeScope::Full | ClusterEndpointSubscribeScope::StreamOnly)
    }

    pub fn is_manual(&self) -> bool {
        matches!(self, ClusterEndpointSubscribeScope::Manual)
    }
}

impl Default for ClusterEndpointSubscribeScope {
    fn default() -> Self {
        ClusterEndpointSubscribeScope::Full
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone, Enum)]
pub enum ClusterEndpointPublishScope {
    Full,
    #[serde(rename = "stream_only")]
    #[oai(rename = "stream_only")]
    StreamOnly,
}

impl ClusterEndpointPublishScope {
    pub fn is_pub_peer(&self) -> bool {
        matches!(self, ClusterEndpointPublishScope::Full)
    }
}

impl Default for ClusterEndpointPublishScope {
    fn default() -> Self {
        ClusterEndpointPublishScope::Full
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ClusterEndpointMeta {
    pub state: ClusterStateEndpointState,
    pub protocol: MediaSessionProtocol,
}

impl ClusterEndpointMeta {
    pub fn new(state: ClusterStateEndpointState, protocol: MediaSessionProtocol) -> Self {
        Self { state, protocol }
    }
}

#[derive(Debug, PartialEq)]
pub enum ClusterEndpointOutgoingEvent {
    InfoSet(ClusterEndpointMeta),
    InfoUpdate(ClusterEndpointMeta),
    InfoRemove,
    SubscribeRoomPeers,
    UnsubscribeRoomPeers,
    SubscribeRoomStreams,
    UnsubscribeRoomStreams,
    SubscribeSinglePeer(ClusterPeerId),
    UnsubscribeSinglePeer(ClusterPeerId),
    LocalTrackEvent(TrackId, ClusterLocalTrackOutgoingEvent),
    RemoteTrackEvent(TrackId, ClusterTrackUuid, ClusterRemoteTrackOutgoingEvent),
    MediaEndpointLog(MediaEndpointLogRequest),
}

#[derive(Clone, PartialEq, Debug)]
pub enum ClusterEndpointIncomingEvent {
    PeerAdded(ClusterPeerId, ClusterEndpointMeta),
    PeerUpdated(ClusterPeerId, ClusterEndpointMeta),
    PeerRemoved(ClusterPeerId),
    PeerTrackAdded(ClusterPeerId, ClusterTrackName, ClusterTrackMeta),
    PeerTrackUpdated(ClusterPeerId, ClusterTrackName, ClusterTrackMeta),
    PeerTrackRemoved(ClusterPeerId, ClusterTrackName),
    LocalTrackEvent(TrackId, ClusterLocalTrackIncomingEvent),
    RemoteTrackEvent(TrackId, ClusterRemoteTrackIncomingEvent),
}
