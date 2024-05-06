use derive_more::{AsRef, From};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

use crate::{
    media::{MediaKind, MediaScaling},
    protobuf,
    transport::ConnLayer,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClusterConnId {
    pub node: u32,
    pub node_session: u64,
    pub server_conn: ServerConnId,
}

impl FromStr for ClusterConnId {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split('-').collect::<Vec<_>>();
        let node = parts.get(0).ok_or("MISSING NODE_ID")?.parse::<u32>().map_err(|_| "PARSE ERROR NODE_ID")?;
        let node_session = parts.get(1).ok_or("MISSING NODE_SESSION")?.parse::<u64>().map_err(|_| "PARSE ERROR NODE_SESSION")?;
        let server_conn = parts.get(2).ok_or("MISSING SERVER_CONN")?.parse::<ServerConnId>().map_err(|_| "PARSE ERROR SERVER_CONN")?;
        Ok(Self { node, node_session, server_conn })
    }
}

impl Display for ClusterConnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.node, self.node_session, self.server_conn)
    }
}

impl ConnLayer for ClusterConnId {
    type Up = ();
    type UpParam = ();
    type Down = ServerConnId;
    type DownRes = (u32, u64);

    fn down(self) -> (Self::Down, Self::DownRes) {
        (self.server_conn, (self.node, self.node_session))
    }

    fn up(self, _param: Self::UpParam) -> Self::Up {
        panic!("should not happen")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServerConnId {
    pub worker: u16,
    pub index: usize,
}

impl FromStr for ServerConnId {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(',').collect::<Vec<_>>();
        let worker = parts.get(0).ok_or("MISSING WORKER")?.parse::<u16>().map_err(|_| "PARSE ERROR WORKER")?;
        let index = parts.get(1).ok_or("MISSING INDEX")?.parse::<usize>().map_err(|_| "PARSE ERROR INDEX")?;
        Ok(Self { worker, index })
    }
}

impl Display for ServerConnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.worker, self.index)
    }
}

impl ConnLayer for ServerConnId {
    type Up = ClusterConnId;
    type UpParam = (u32, u64);
    type Down = usize;
    type DownRes = u16;

    fn down(self) -> (Self::Down, Self::DownRes) {
        (self.index, self.worker)
    }

    fn up(self, param: Self::UpParam) -> Self::Up {
        ClusterConnId {
            node: param.0,
            node_session: param.1,
            server_conn: self,
        }
    }
}

impl ConnLayer for usize {
    type Up = ServerConnId;
    type UpParam = u16;
    type Down = ();
    type DownRes = ();

    fn down(self) -> (Self::Down, Self::DownRes) {
        panic!("should not happen")
    }

    fn up(self, param: Self::UpParam) -> Self::Up {
        ServerConnId { index: self, worker: param }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomInfoPublish {
    pub peer: bool,
    pub tracks: bool,
}

impl From<protobuf::shared::RoomInfoPublish> for RoomInfoPublish {
    fn from(value: protobuf::shared::RoomInfoPublish) -> Self {
        Self {
            peer: value.peer,
            tracks: value.tracks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomInfoSubscribe {
    pub peers: bool,
    pub tracks: bool,
}

impl From<protobuf::shared::RoomInfoSubscribe> for RoomInfoSubscribe {
    fn from(value: protobuf::shared::RoomInfoSubscribe) -> Self {
        Self {
            peers: value.peers,
            tracks: value.tracks,
        }
    }
}

#[derive(From, AsRef, Debug, derive_more::Display, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub String);

impl From<&str> for RoomId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(From, AsRef, Debug, derive_more::Display, derive_more::FromStr, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub String);

impl From<&str> for PeerId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerMeta {
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer: PeerId,
    pub meta: PeerMeta,
}

impl PeerInfo {
    pub fn new(peer: PeerId, meta: PeerMeta) -> Self {
        Self { peer, meta }
    }
}

impl PeerInfo {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("should ok")
    }

    pub fn deserialize(data: &[u8]) -> Option<PeerInfo> {
        bincode::deserialize::<Self>(data).ok()
    }
}

#[derive(From, AsRef, Debug, derive_more::Display, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackName(pub String);

impl From<&str> for TrackName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(From, AsRef, Debug, derive_more::Display, derive_more::Add, derive_more::AddAssign, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackPriority(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMeta {
    pub kind: MediaKind,
    pub scaling: MediaScaling,
    pub control: Option<BitrateControlMode>,
    pub metadata: Option<String>,
}

impl TrackMeta {
    pub fn default_audio() -> Self {
        Self {
            kind: MediaKind::Audio,
            scaling: MediaScaling::None,
            control: None,
            metadata: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub peer: PeerId,
    pub track: TrackName,
    pub meta: TrackMeta,
}

impl TrackInfo {
    pub fn simple_audio(peer: PeerId) -> Self {
        Self {
            peer,
            track: "audio_main".to_string().into(),
            meta: TrackMeta::default_audio(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("should ok")
    }

    pub fn deserialize(data: &[u8]) -> Option<TrackInfo> {
        bincode::deserialize::<Self>(data).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitrateControlMode {
    /// Only limit with sender network and CAP with fixed MAX_BITRATE
    MaxBitrate,
    /// Calc limit based on MAX_BITRATE and consumers requested bitrate
    DynamicConsumers,
}

impl From<protobuf::shared::BitrateControlMode> for BitrateControlMode {
    fn from(value: protobuf::shared::BitrateControlMode) -> Self {
        match value {
            protobuf::shared::BitrateControlMode::MaxBitrate => Self::MaxBitrate,
            protobuf::shared::BitrateControlMode::DynamicConsumers => Self::DynamicConsumers,
        }
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::{ClusterConnId, ServerConnId};

    #[test]
    fn server_conn_id_parse() {
        let conn = ServerConnId { worker: 1, index: 2 };
        assert_eq!(conn.to_string(), "1,2");
        assert_eq!(ServerConnId::from_str("1,2"), Ok(ServerConnId { worker: 1, index: 2 }));
    }

    #[test]
    fn cluster_conn_id_parse() {
        let conn = ClusterConnId {
            node: 1,
            node_session: 2,
            server_conn: ServerConnId { worker: 3, index: 4 },
        };
        assert_eq!(conn.to_string(), "1-2-3,4");
        assert_eq!(
            ClusterConnId::from_str("1-2-3,4"),
            Ok(ClusterConnId {
                node: 1,
                node_session: 2,
                server_conn: ServerConnId { worker: 3, index: 4 },
            })
        );
    }
}
