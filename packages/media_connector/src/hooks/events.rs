use atm0s_sdn::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "reconnect")]
    Reconnect,
    #[serde(rename = "disconnected")]
    Disconnected,
    #[serde(rename = "reconnected")]
    Reconnected,
    #[serde(rename = "connect_error")]
    ConnectError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookSessionEventPayload {
    pub session: u64,
    pub state: SessionState,
    pub remote_ip: Option<String>,
    pub after_ms: Option<u32>,
    pub duration: Option<u32>,
    pub reason: Option<i32>,
    pub error: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerEvent {
    #[serde(rename = "peer_joined")]
    Joined,
    #[serde(rename = "peer_leaved")]
    Leaved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookPeerEventPayload {
    pub session: u64,
    pub peer: String,
    pub room: String,
    pub event: PeerEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RemoteTrackEvent {
    #[serde(rename = "remote_track_started")]
    Started,
    #[serde(rename = "remote_track_ended")]
    Ended,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookRemoteTrackEventPayload {
    pub session: u64,
    pub track: String,
    pub kind: i32,
    pub event: RemoteTrackEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LocalTrackEvent {
    #[serde(rename = "local_track")]
    LocalTrack,
    #[serde(rename = "local_track_attached")]
    Attached,
    #[serde(rename = "local_track_detached")]
    Detached,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookLocalTrackEventPayload {
    pub session: u64,
    pub track: i32,
    pub event: LocalTrackEvent,
    pub kind: Option<i32>,
    pub remote_peer: Option<String>,
    pub remote_track: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HookEvent {
    Session {
        uuid: String,
        node: NodeId,
        ts: u64,
        session: u64,
        state: SessionState,
        remote_ip: Option<String>,
        after_ms: Option<u32>,
        duration: Option<u32>,
        reason: Option<i32>,
        error: Option<i32>,
    },
    Peer {
        uuid: String,
        node: NodeId,
        ts: u64,
        session: u64,
        room: String,
        peer: String,
        event: PeerEvent,
    },
    RemoteTrack {
        uuid: String,
        node: NodeId,
        ts: u64,
        session: u64,
        track: String,
        kind: i32,
        event: RemoteTrackEvent,
    },
    LocalTrack {
        uuid: String,
        node: NodeId,
        ts: u64,
        session: u64,
        track: i32,
        event: LocalTrackEvent,
        kind: Option<i32>,
        remote_peer: Option<String>,
        remote_track: Option<String>,
    },
}

impl HookEvent {
    pub fn id(&self) -> &str {
        match self {
            HookEvent::Session { uuid, .. } => uuid,
            HookEvent::Peer { uuid, .. } => uuid,
            HookEvent::RemoteTrack { uuid, .. } => uuid,
            HookEvent::LocalTrack { uuid, .. } => uuid,
        }
    }
}

impl<'de> Deserialize<'de> for HookEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        pub struct DataInner {
            pub uuid: String,
            pub node: u32,
            pub ts: u64,
            pub event: String,
            pub payload: serde_json::Value,
        }

        let data = DataInner::deserialize(deserializer)?;
        match data.event.as_str() {
            "session" => {
                let payload: HookSessionEventPayload = serde_json::from_value(data.payload).map_err(serde::de::Error::custom)?;
                Ok(HookEvent::Session {
                    uuid: data.uuid,
                    node: data.node,
                    ts: data.ts,
                    session: payload.session,
                    state: payload.state,
                    remote_ip: payload.remote_ip,
                    after_ms: payload.after_ms,
                    duration: payload.duration,
                    reason: payload.reason,
                    error: payload.error,
                })
            }
            "peer" => {
                let payload: HookPeerEventPayload = serde_json::from_value(data.payload).map_err(serde::de::Error::custom)?;
                Ok(HookEvent::Peer {
                    uuid: data.uuid,
                    node: data.node,
                    ts: data.ts,
                    session: payload.session,
                    room: payload.room,
                    peer: payload.peer,
                    event: payload.event,
                })
            }
            "remote_track" => {
                let payload: HookRemoteTrackEventPayload = serde_json::from_value(data.payload).map_err(serde::de::Error::custom)?;
                Ok(HookEvent::RemoteTrack {
                    uuid: data.uuid,
                    node: data.node,
                    ts: data.ts,
                    session: payload.session,
                    track: payload.track,
                    kind: payload.kind,
                    event: payload.event,
                })
            }
            "local_track" => {
                let payload: HookLocalTrackEventPayload = serde_json::from_value(data.payload).map_err(serde::de::Error::custom)?;
                Ok(HookEvent::LocalTrack {
                    uuid: data.uuid,
                    node: data.node,
                    ts: data.ts,
                    session: payload.session,
                    track: payload.track,
                    event: payload.event,
                    kind: payload.kind,
                    remote_peer: payload.remote_peer,
                    remote_track: payload.remote_track,
                })
            }
            _ => Err(serde::de::Error::custom(format!("unknown event: {}", data.event))),
        }
    }
}

impl Serialize for HookEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize, Debug)]
        pub struct DataInner {
            uuid: String,
            pub node: u32,
            pub ts: u64,
            pub event: String,
            pub payload: serde_json::Value,
        }
        match self {
            HookEvent::Session {
                uuid,
                node,
                ts,
                session,
                state,
                remote_ip,
                after_ms,
                duration,
                reason,
                error,
            } => {
                let payload = HookSessionEventPayload {
                    session: *session,
                    state: *state,
                    remote_ip: remote_ip.clone(),
                    after_ms: *after_ms,
                    duration: *duration,
                    reason: *reason,
                    error: *error,
                };
                let payload = serde_json::to_value(payload).map_err(serde::ser::Error::custom)?;
                let data = DataInner {
                    uuid: uuid.clone(),
                    node: *node,
                    ts: *ts,
                    event: "session".to_string(),
                    payload,
                };
                data.serialize(serializer)
            }
            HookEvent::Peer {
                uuid,
                node,
                ts,
                session,
                room,
                peer,
                event,
            } => {
                let payload = HookPeerEventPayload {
                    session: *session,
                    peer: peer.clone(),
                    room: room.clone(),
                    event: event.clone(),
                };
                let payload = serde_json::to_value(payload).map_err(serde::ser::Error::custom)?;
                let data = DataInner {
                    uuid: uuid.clone(),
                    node: *node,
                    ts: *ts,
                    event: "peer".to_string(),
                    payload,
                };
                data.serialize(serializer)
            }
            HookEvent::RemoteTrack {
                uuid,
                node,
                ts,
                session,
                track,
                kind,
                event,
            } => {
                let payload = HookRemoteTrackEventPayload {
                    session: *session,
                    track: track.clone(),
                    kind: *kind,
                    event: event.clone(),
                };
                let payload = serde_json::to_value(payload).map_err(serde::ser::Error::custom)?;
                let data = DataInner {
                    uuid: uuid.clone(),
                    node: *node,
                    ts: *ts,
                    event: "remote_track".to_string(),
                    payload,
                };
                data.serialize(serializer)
            }
            HookEvent::LocalTrack {
                uuid,
                node,
                ts,
                session,
                track,
                event,
                kind,
                remote_peer,
                remote_track,
            } => {
                let payload = HookLocalTrackEventPayload {
                    session: *session,
                    track: *track,
                    event: event.clone(),
                    kind: *kind,
                    remote_peer: remote_peer.clone(),
                    remote_track: remote_track.clone(),
                };
                let payload = serde_json::to_value(payload).map_err(serde::ser::Error::custom)?;
                let data = DataInner {
                    uuid: uuid.clone(),
                    node: *node,
                    ts: *ts,
                    event: "local_track".to_string(),
                    payload,
                };
                data.serialize(serializer)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_session_event() {
        let data = r#"{"uuid":"67e55044-10b1-426f-9247-bb680e5fe0c8","node":1,"ts":1,"event":"session","payload":{"after_ms":null,"duration":null,"error":null,"reason":null,"remote_ip":"127.0.0.1","session":1,"state":"connecting"}}"#;
        let event = serde_json::from_str::<HookEvent>(data).unwrap();
        assert_eq!(
            event,
            HookEvent::Session {
                uuid: "67e55044-10b1-426f-9247-bb680e5fe0c8".to_string(),
                node: 1,
                ts: 1,
                session: 1,
                state: SessionState::Connecting,
                remote_ip: Some("127.0.0.1".to_string()),
                after_ms: None,
                duration: None,
                error: None,
                reason: None,
            }
        );
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, data);
    }

    #[test]
    pub fn test_peer_event() {
        let data = r#"{"uuid":"67e55044-10b1-426f-9247-bb680e5fe0c8","node":1,"ts":1,"event":"peer","payload":{"event":"peer_joined","peer":"peer","room":"room","session":1}}"#;
        let event = serde_json::from_str::<HookEvent>(data).unwrap();
        assert_eq!(
            event,
            HookEvent::Peer {
                uuid: "67e55044-10b1-426f-9247-bb680e5fe0c8".to_string(),
                node: 1,
                ts: 1,
                session: 1,
                room: "room".to_string(),
                peer: "peer".to_string(),
                event: PeerEvent::Joined
            }
        );
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, data);
    }

    #[test]
    pub fn test_remote_track() {
        let data = r#"{"uuid":"67e55044-10b1-426f-9247-bb680e5fe0c8","node":1,"ts":1,"event":"remote_track","payload":{"event":"remote_track_started","kind":1,"session":1,"track":"track"}}"#;
        let event = serde_json::from_str::<HookEvent>(data).unwrap();
        assert_eq!(
            event,
            HookEvent::RemoteTrack {
                uuid: "67e55044-10b1-426f-9247-bb680e5fe0c8".to_string(),
                node: 1,
                ts: 1,
                session: 1,
                track: "track".to_string(),
                kind: 1,
                event: RemoteTrackEvent::Started
            }
        );
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, data);
    }

    #[test]
    pub fn test_local_track() {
        let data = r#"{"uuid":"67e55044-10b1-426f-9247-bb680e5fe0c8","node":1,"ts":1,"event":"local_track","payload":{"event":"local_track","kind":1,"remote_peer":null,"remote_track":"track","session":1,"track":1}}"#;
        let event = serde_json::from_str::<HookEvent>(data).unwrap();
        assert_eq!(
            event,
            HookEvent::LocalTrack {
                uuid: "67e55044-10b1-426f-9247-bb680e5fe0c8".to_string(),
                node: 1,
                ts: 1,
                session: 1,
                track: 1,
                event: LocalTrackEvent::LocalTrack,
                kind: Some(1),
                remote_peer: None,
                remote_track: Some("track".to_string()),
            }
        );
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, data);
    }
}
