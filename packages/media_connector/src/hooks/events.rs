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
    #[serde(rename = "joined")]
    Joined,
    #[serde(rename = "leaved")]
    Leaved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookPeerEventPayload {
    pub session: u64,
    pub peer: String,
    pub room: String,
    pub event: PeerEvent,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HookEvent {
    Session {
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
        node: NodeId,
        ts: u64,
        session: u64,
        room: String,
        peer: String,
        event: PeerEvent,
    },
}

impl<'de> Deserialize<'de> for HookEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        pub struct DataInner {
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
                    node: data.node,
                    ts: data.ts,
                    session: payload.session,
                    room: payload.room,
                    peer: payload.peer,
                    event: payload.event,
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
            pub node: u32,
            pub ts: u64,
            pub event: String,
            pub payload: serde_json::Value,
        }
        match self {
            HookEvent::Session {
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
                    node: *node,
                    ts: *ts,
                    event: "session".to_string(),
                    payload,
                };
                data.serialize(serializer)
            }
            HookEvent::Peer { node, ts, session, room, peer, event } => {
                let payload = HookPeerEventPayload {
                    session: *session,
                    peer: peer.clone(),
                    room: room.clone(),
                    event: event.clone(),
                };
                let payload = serde_json::to_value(payload).map_err(serde::ser::Error::custom)?;
                let data = DataInner {
                    node: *node,
                    ts: *ts,
                    event: "peer".to_string(),
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
        let data = r#"{"node":1,"ts":1,"event":"session","payload":{"after_ms":null,"duration":null,"error":null,"reason":null,"remote_ip":"127.0.0.1","session":1,"state":"connecting"}}"#;
        let event = serde_json::from_str::<HookEvent>(data).unwrap();
        assert_eq!(
            event,
            HookEvent::Session {
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
        let data = r#"{"node":1,"ts":1,"event":"peer","payload":{"event":"joined","peer":"peer","room":"room","session":1}}"#;
        let event = serde_json::from_str::<HookEvent>(data).unwrap();
        assert_eq!(
            event,
            HookEvent::Peer {
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
}
