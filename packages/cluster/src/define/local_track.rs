use transport::{MediaPacket, RequestKeyframeKind};

use crate::{ClusterPeerId, ClusterTrackName, ClusterTrackStats};

#[derive(Debug, PartialEq, Eq)]
pub enum ClusterLocalTrackOutgoingEvent {
    RequestKeyFrame(RequestKeyframeKind),
    LimitBitrate(u32),
    Subscribe(ClusterPeerId, ClusterTrackName),
    Unsubscribe(ClusterPeerId, ClusterTrackName),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClusterLocalTrackIncomingEvent {
    MediaPacket(MediaPacket),
    MediaStats(ClusterTrackStats),
}
