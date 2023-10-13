use cluster::{ClusterConsumerId, ClusterTrackUuid};
use transport::{MediaPacket, TrackId, TrackMeta};
use utils::hash_str;

use crate::rpc::BitrateLimit;

pub fn local_track_consumer_id(room: &str, peer: &str, local_track: &str) -> u64 {
    hash_str(&format!("{}-{}-{}", room, peer, local_track))
}

pub struct LocalTrackSource {
    pub(crate) peer: String,
    pub(crate) track: String,
    pub(crate) uuid: ClusterTrackUuid,
}

impl LocalTrackSource {
    pub fn new(peer: &str, track: &str, uuid: ClusterTrackUuid) -> Self {
        Self {
            peer: peer.into(),
            track: track.into(),
            uuid,
        }
    }
}

pub enum LocalTrackOutput {}

pub struct LocalTrack {
    consumer_uuid: ClusterConsumerId,
    room_id: String,
    peer_id: String,
    track_id: TrackId,
    track_name: String,
    track_meta: TrackMeta,
    source: Option<LocalTrackSource>,
}

impl LocalTrack {
    pub fn new(room_id: &str, peer_id: &str, track_id: TrackId, track_name: &str, track_meta: TrackMeta) -> Self {
        let consumer_uuid = local_track_consumer_id(room_id, peer_id, track_name);
        Self {
            consumer_uuid,
            room_id: room_id.into(),
            peer_id: peer_id.into(),
            track_id,
            track_name: track_name.into(),
            track_meta,
            source: None,
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {}

    pub fn limit(&mut self, limit: BitrateLimit) {}

    pub fn consumer_uuid(&self) -> ClusterConsumerId {
        self.consumer_uuid
    }

    pub fn source_uuid(&self) -> Option<ClusterTrackUuid> {
        self.source.as_ref().map(|s| s.uuid)
    }

    /// Replace to new source, return old source
    pub fn repace_source(&mut self, source: Option<LocalTrackSource>) -> Option<LocalTrackSource> {
        let old = self.source.take();
        self.source = source;
        old
    }

    pub fn on_pkt(&mut self, pkt: &MediaPacket) -> Option<(u16, MediaPacket)> {
        Some((self.track_id, pkt.clone()))
    }

    pub fn pop_action(&mut self) -> Option<LocalTrackOutput> {
        None
    }
}
