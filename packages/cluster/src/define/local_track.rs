use transport::{MediaPacket, RequestKeyframeKind};

use crate::{ClusterPeerId, ClusterTrackName, ClusterTrackStats, ClusterTrackUuid};

#[derive(Debug, PartialEq, Eq)]
pub enum ClusterLocalTrackOutgoingEvent {
    RequestKeyFrame(RequestKeyframeKind),
    LimitBitrate(u32),
    Subscribe(ClusterPeerId, ClusterTrackName),
    Unsubscribe(ClusterPeerId, ClusterTrackName),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClusterLocalTrackIncomingEvent {
    MediaPacket(ClusterTrackUuid, MediaPacket),
    MediaStats(ClusterTrackUuid, ClusterTrackStats),
}
