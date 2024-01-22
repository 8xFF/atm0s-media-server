use cluster::{ClusterEndpointMeta, ClusterTrackMeta};
use media_utils::hash_str;
use serde::{Deserialize, Serialize};
use transport::MediaKind;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct RpcRequest<D> {
    pub req_id: u64,
    pub data: D,
}

impl<D> RpcRequest<D> {
    pub fn from(req_id: u64, data: D) -> Self {
        Self { req_id, data }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct RpcResponse<D> {
    pub req_id: u64,
    #[serde(rename = "type")]
    _type: String,
    pub success: bool,
    pub data: Option<D>,
}

impl<D> RpcResponse<D> {
    pub fn success(req_id: u64, data: D) -> Self {
        Self {
            req_id,
            _type: "answer".to_string(),
            success: true,
            data: Some(data),
        }
    }

    pub fn error(req_id: u64) -> Self {
        Self {
            req_id,
            _type: "answer".to_string(),
            success: false,
            data: None,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct RemotePeer {
    pub peer: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct RemoteStream {
    pub peer: String,
    pub stream: String,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct ReceiverLayerLimit {
    pub priority: u16,
    pub min_spatial: Option<u8>,
    pub max_spatial: u8,
    pub min_temporal: Option<u8>,
    pub max_temporal: u8,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct SenderToggle {
    pub name: String,
    pub kind: MediaKind,
    pub track: Option<String>,
    pub label: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct ReceiverSwitch {
    pub id: String,
    pub priority: u16,
    pub remote: RemoteStream,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct MixMinusSource {
    pub id: String,
    pub remote: RemoteStream,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct MixMinusToggle {
    pub id: String,
    pub enable: bool,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct ReceiverLimit {
    pub id: String,
    pub limit: ReceiverLayerLimit,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct ReceiverDisconnect {
    pub id: String,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct PeerInfo {
    peer_hash: u32,
    pub peer: String,
    pub state: Option<ClusterEndpointMeta>,
}

impl PeerInfo {
    pub fn new(peer: &str, state: Option<ClusterEndpointMeta>) -> Self {
        Self {
            peer_hash: hash_str(peer) as u32,
            peer: peer.to_string(),
            state,
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TrackInfo {
    peer_hash: u32,
    pub peer: String,
    pub kind: MediaKind,
    #[serde(rename = "stream")]
    pub track: String,
    pub state: Option<ClusterTrackMeta>,
}

impl TrackInfo {
    pub fn new(peer: &str, track: &str, kind: MediaKind, state: Option<ClusterTrackMeta>) -> Self {
        Self {
            peer_hash: hash_str(peer) as u32,
            peer: peer.to_string(),
            kind,
            track: track.to_string(),
            state,
        }
    }

    pub fn new_audio(peer: &str, track: &str, state: Option<ClusterTrackMeta>) -> Self {
        Self::new(peer, track, MediaKind::Audio, state)
    }

    pub fn new_video(peer: &str, track: &str, state: Option<ClusterTrackMeta>) -> Self {
        Self::new(peer, track, MediaKind::Video, state)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointRpcIn {
    PeerClose,
    SubscribePeer(RpcRequest<RemotePeer>),
    UnsubscribePeer(RpcRequest<RemotePeer>),
    MixMinusSourceAdd(RpcRequest<MixMinusSource>),
    MixMinusSourceRemove(RpcRequest<MixMinusSource>),
    MixMinusToggle(RpcRequest<MixMinusToggle>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RemoteTrackRpcIn {
    Toggle(RpcRequest<SenderToggle>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum LocalTrackRpcIn {
    Switch(RpcRequest<ReceiverSwitch>),
    Limit(RpcRequest<ReceiverLimit>),
    Disconnect(RpcRequest<ReceiverDisconnect>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointRpcOut {
    MixMinusSourceAddRes(RpcResponse<bool>),
    MixMinusSourceRemoveRes(RpcResponse<bool>),
    MixMinusToggleRes(RpcResponse<bool>),
    PeerAdded(PeerInfo),
    PeerUpdated(PeerInfo),
    PeerRemoved(PeerInfo),
    TrackAdded(TrackInfo),
    TrackUpdated(TrackInfo),
    TrackRemoved(TrackInfo),
    SubscribePeerRes(RpcResponse<bool>),
    UnsubscribePeerRes(RpcResponse<bool>),
    ConnectionAcceptRequest, //Use for calling style rooms
}

#[derive(Debug, PartialEq, Eq)]
pub enum RemoteTrackRpcOut {
    ToggleRes(RpcResponse<bool>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum LocalTrackRpcOut {
    SwitchRes(RpcResponse<bool>),
    LimitRes(RpcResponse<bool>),
    DisconnectRes(RpcResponse<bool>),
}
