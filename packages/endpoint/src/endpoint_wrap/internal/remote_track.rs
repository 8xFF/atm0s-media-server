use std::collections::VecDeque;

use cluster::{generate_cluster_track_uuid, ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent, ClusterTrackMeta, ClusterTrackScalingType, ClusterTrackStatus, ClusterTrackUuid};
use transport::{RemoteTrackIncomingEvent, RemoteTrackOutgoingEvent, TrackId, TrackMeta};

use crate::{
    rpc::{RemoteTrackRpcIn, RemoteTrackRpcOut},
    RpcResponse,
};

#[derive(PartialEq, Eq, Debug)]
pub enum RemoteTrackOutput {
    Transport(RemoteTrackOutgoingEvent<RemoteTrackRpcOut>),
    Cluster(ClusterRemoteTrackOutgoingEvent),
}

#[allow(dead_code)]
pub struct RemoteTrack {
    cluster_track_uuid: ClusterTrackUuid,
    track_id: TrackId,
    track_name: String,
    track_meta: TrackMeta,
    out_actions: VecDeque<RemoteTrackOutput>,
}

impl RemoteTrack {
    pub fn new(room_id: &str, peer_id: &str, track_id: TrackId, track_name: &str, track_meta: TrackMeta) -> Self {
        Self {
            cluster_track_uuid: generate_cluster_track_uuid(room_id, peer_id, track_name),
            track_id,
            track_name: track_name.into(),
            track_meta,
            out_actions: Default::default(),
        }
    }

    pub fn track_name(&self) -> &str {
        &self.track_name
    }

    pub fn cluster_track_uuid(&self) -> ClusterTrackUuid {
        self.cluster_track_uuid
    }

    pub fn cluster_meta(&self) -> ClusterTrackMeta {
        ClusterTrackMeta {
            kind: self.track_meta.kind,
            scaling: ClusterTrackScalingType::Single,
            layers: vec![],
            status: ClusterTrackStatus::Connected,
            active: true,
            label: None,
        }
    }

    pub fn on_tick(&mut self, _now_ms: u64) {}

    pub fn on_cluster_event(&mut self, event: ClusterRemoteTrackIncomingEvent) {
        match event {
            ClusterRemoteTrackIncomingEvent::RequestKeyFrame => {
                self.out_actions.push_back(RemoteTrackOutput::Transport(RemoteTrackOutgoingEvent::RequestKeyFrame));
            }
            ClusterRemoteTrackIncomingEvent::RequestLimitBitrate(_) => {
                //TODO
            }
        }
    }

    pub fn on_transport_event(&mut self, event: RemoteTrackIncomingEvent<RemoteTrackRpcIn>) {
        match event {
            RemoteTrackIncomingEvent::MediaPacket(pkt) => {
                log::debug!("[RemoteTrack {}] media from transport pkt {} {}", self.track_name, pkt.pt, pkt.seq_no);
                self.out_actions.push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt)));
            }
            RemoteTrackIncomingEvent::Rpc(event) => match event {
                RemoteTrackRpcIn::Toggle(req) => {
                    self.out_actions
                        .push_back(RemoteTrackOutput::Transport(RemoteTrackOutgoingEvent::Rpc(RemoteTrackRpcOut::ToggleRes(RpcResponse::success(
                            req.req_id, true,
                        )))));
                }
            },
        }
    }

    pub fn pop_action(&mut self) -> Option<RemoteTrackOutput> {
        self.out_actions.pop_front()
    }

    /// Close this and cleanup everything
    /// This should be called when the track is removed from the peer
    pub fn close(&mut self) {}
}

#[cfg(test)]
mod tests {
    use cluster::ClusterRemoteTrackIncomingEvent;
    use transport::{MediaKind, MediaPacket, MediaPacketExtensions, MediaSampleRate, TrackMeta};

    use crate::endpoint_wrap::internal::remote_track::RemoteTrackOutput;

    use super::RemoteTrack;

    #[test]
    fn incoming_media_should_fire_cluster_media() {
        let mut track = RemoteTrack::new("room1", "peer1", 100, "video_main", TrackMeta::new_audio(None));

        let pkt = MediaPacket::default_audio(1, 1000, vec![1, 2, 3]);
        track.on_transport_event(transport::RemoteTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Cluster(cluster::ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt))));
        assert_eq!(track.pop_action(), None);
    }

    #[test]
    fn incoming_request_keyframe_should_fire_transport_event() {
        let mut track = RemoteTrack::new("room1", "peer1", 100, "video_main", TrackMeta::new_audio(None));

        track.on_cluster_event(ClusterRemoteTrackIncomingEvent::RequestKeyFrame);
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Transport(transport::RemoteTrackOutgoingEvent::RequestKeyFrame)));
        assert_eq!(track.pop_action(), None);
    }
}
