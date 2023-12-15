use std::net::SocketAddr;

use atm0s_sdn::NodeId;
use media_utils::F32;
use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};
use transport::MediaKind;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub enum MediaStreamIssueType {
    Connectivity { mos: F32<2>, lost_percents: F32<2>, jitter_ms: F32<2>, rtt_ms: u32 },
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
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
        rtt: F32<2>,
    },
    SessionStats {
        received_bytes: u64,
        receive_limit_bitrate: u32,
        sent_bytes: u64,
        send_est_bitrate: u32,
        rtt: u16,
    },
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
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
        mos: Option<F32<2>>,
        rtt: Option<u32>,
        jitter: Option<F32<2>>,
        lost: Option<F32<2>>,
    },
    StreamEnded {
        name: String,
        kind: MediaKind,
        sent_bytes: u64,
        freeze_count: u32,
        duration_ms: u64,
        mos: Option<(F32<2>, F32<2>, F32<2>)>,
        rtt: Option<(F32<2>, F32<2>, F32<2>)>,
        jitter: Option<(F32<2>, F32<2>, F32<2>)>,
        lost: Option<(F32<2>, F32<2>, F32<2>)>,
    },
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
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
        mos: Option<F32<2>>,
        rtt: Option<u32>,
        jitter: Option<F32<2>>,
        lost: Option<F32<2>>,
    },
    StreamEnded {
        name: String,
        kind: MediaKind,
        received_bytes: u64,
        duration_ms: u64,
        freeze_count: u32,
        mos: Option<(F32<2>, F32<2>, F32<2>)>,
        rtt: Option<(F32<2>, F32<2>, F32<2>)>,
        jitter: Option<(F32<2>, F32<2>, F32<2>)>,
        lost: Option<(F32<2>, F32<2>, F32<2>)>,
    },
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, IntoVecU8, TryFromSliceU8)]
pub enum MediaEndpointLogRequest {
    SessionEvent {
        ip: String,
        version: Option<String>,
        location: Option<(F32<2>, F32<2>)>,
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
