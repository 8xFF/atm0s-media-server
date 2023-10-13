use cluster::{ClusterTrackMeta, ClusterTrackStatus};
use transport::{MediaKind, MediaPacket, TrackId, TrackMeta};

pub enum RemoteTrackOutput {}

pub struct RemoteTrack {
    track_id: TrackId,
    track_name: String,
    track_meta: TrackMeta,
    cluster_track_uuid: u64,
}

impl RemoteTrack {
    pub fn new(track_id: TrackId, track_name: &str, track_meta: TrackMeta) -> Self {
        Self {
            track_id,
            track_name: track_name.into(),
            track_meta,
            cluster_track_uuid: 0, //TODO
        }
    }

    pub fn cluster_meta(&self) -> ClusterTrackMeta {
        ClusterTrackMeta {
            kind: self.track_meta.kind,
            scaling: "Single".to_string(),
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            active: true,
            label: None,
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {}

    pub fn on_pkt(&mut self, pkt: MediaPacket) -> Option<(u64, MediaPacket)> {
        Some((self.cluster_track_uuid, pkt))
    }

    pub fn pop_action(&mut self) -> Option<RemoteTrackOutput> {
        None
    }
}
