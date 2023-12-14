use std::net::SocketAddr;

use atm0s_sdn::NodeId;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};
use transport::MediaKind;

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MediaStreamIssueType {
    Connectivity { mos: f32, lost_percents: f32, jitter_ms: f32, rtt_ms: u32 },
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MediaEndpointEvent {
    Routing {
        user_agent: String,
        gateway_node_id: NodeId,
    },
    RoutingError {
        reason: String,
        gateway_node_id: NodeId,
        media_node_ids: Vec<NodeId>,
    },
    Routed {
        media_node_id: NodeId,
        after_ms: u32,
    },
    Connecting {
        user_agent: String,
        remote: Option<SocketAddr>,
    },
    ConnectError {
        remote: Option<SocketAddr>,
        error_code: String,
        error_message: String,
    },
    Connected {
        after_ms: u32,
        remote: Option<SocketAddr>,
    },
    Reconnecting {
        reason: String,
    },
    Reconnected {
        remote: Option<SocketAddr>,
    },
    Disconnected {
        error: Option<String>,
        sent_bytes: u64,
        received_bytes: u64,
        duration_ms: u64,
        rtt: f32,
    },
    SessionStats {
        received_bytes: u64,
        receive_limit_bitrate: u32,
        sent_bytes: u64,
        send_est_bitrate: u32,
        rtt: u16,
    },
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MediaReceiveStreamEvent {
    StreamStarted {
        name: String,
        kind: MediaKind,
        remote_peer: String,
        remote_stream: String,
    },
    StreamIssue {
        name: String,
        kind: MediaKind,
        remote_peer: String,
        remote_stream: String,
        issue: MediaStreamIssueType,
    },
    StreamStats {
        name: String,
        kind: MediaKind,
        limit_bitrate: u32,
        received_bytes: u64,
        freeze: bool,
        mos: Option<f32>,
        rtt: Option<u32>,
        jitter: Option<f32>,
        lost: Option<f32>,
    },
    StreamEnded {
        name: String,
        kind: MediaKind,
        sent_bytes: u64,
        freeze_count: u32,
        duration_ms: u64,
        mos: Option<(f32, f32, f32)>,
        rtt: Option<(f32, f32, f32)>,
        jitter: Option<(f32, f32, f32)>,
        lost: Option<(f32, f32, f32)>,
    },
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MediaSendStreamEvent {
    StreamStarted {
        name: String,
        kind: MediaKind,
        meta: String,
        scaling: String,
    },
    StreamIssue {
        name: String,
        kind: MediaKind,
        issue: MediaStreamIssueType,
    },
    StreamStats {
        name: String,
        kind: MediaKind,
        sent_bytes: u64,
        freeze: bool,
        mos: Option<f32>,
        rtt: Option<u32>,
        jitter: Option<f32>,
        lost: Option<f32>,
    },
    StreamEnded {
        name: String,
        kind: MediaKind,
        received_bytes: u64,
        duration_ms: u64,
        freeze_count: u32,
        mos: Option<(f32, f32, f32)>,
        rtt: Option<(f32, f32, f32)>,
        jitter: Option<(f32, f32, f32)>,
        lost: Option<(f32, f32, f32)>,
    },
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, IntoVecU8, TryFromSliceU8)]
pub enum MediaEndpointLogRequest {
    SessionEvent {
        ip: String,
        version: Option<String>,
        location: Option<(f64, f64)>,
        token: Vec<u8>,
        ts: u64,
        session_uuid: u64,
        event: MediaEndpointEvent,
    },
    ReceiveStreamEvent {
        token: Vec<u8>,
        ts: u64,
        session_uuid: u64,
        event: MediaReceiveStreamEvent,
    },
    SendStreamEvent {
        token: Vec<u8>,
        ts: u64,
        session_uuid: u64,
        event: MediaSendStreamEvent,
    },
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, IntoVecU8, TryFromSliceU8)]
pub struct MediaEndpointLogResponse {}
