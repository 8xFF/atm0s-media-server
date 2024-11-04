use std::collections::HashMap;

use media_server_protocol::{
    media::MediaKind,
    protobuf::{cluster_connector::compose_event::record_job_completed, shared::Kind},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TrackTimeline {
    pub path: String,
    pub start: u64,
    pub end: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TrackSummary {
    pub kind: MediaKind,
    pub timeline: Vec<TrackTimeline>,
}

#[derive(Debug, Default, Serialize)]
pub struct SessionSummary {
    pub track: HashMap<String, TrackSummary>,
}

#[derive(Debug, Default, Serialize)]
pub struct PeerSummary {
    pub sessions: HashMap<u64, SessionSummary>,
}

#[derive(Debug, Default, Serialize)]
pub struct RecordSummary {
    pub peers: HashMap<String, PeerSummary>,
}

impl From<TrackTimeline> for record_job_completed::TrackTimeline {
    fn from(value: TrackTimeline) -> Self {
        record_job_completed::TrackTimeline {
            path: value.path,
            start: value.start,
            end: value.end.unwrap_or_default(),
        }
    }
}

impl From<TrackSummary> for record_job_completed::TrackSummary {
    fn from(value: TrackSummary) -> Self {
        record_job_completed::TrackSummary {
            kind: Kind::from(value.kind) as i32,
            timeline: value.timeline.into_iter().map(|t| t.into()).collect(),
        }
    }
}

impl From<SessionSummary> for record_job_completed::SessionSummary {
    fn from(value: SessionSummary) -> Self {
        record_job_completed::SessionSummary {
            track: value.track.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl From<PeerSummary> for record_job_completed::PeerSummary {
    fn from(value: PeerSummary) -> Self {
        record_job_completed::PeerSummary {
            sessions: value.sessions.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl From<RecordSummary> for record_job_completed::RecordSummary {
    fn from(value: RecordSummary) -> Self {
        record_job_completed::RecordSummary {
            peers: value.peers.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}
