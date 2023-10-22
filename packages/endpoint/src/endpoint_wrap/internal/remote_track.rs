use cluster::{generate_cluster_track_uuid, ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent, ClusterTrackMeta, ClusterTrackScalingType, ClusterTrackStatus, ClusterTrackUuid};
use std::collections::VecDeque;
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
    active: bool,
}

impl RemoteTrack {
    pub fn new(room_id: &str, peer_id: &str, track_id: TrackId, track_name: &str, track_meta: TrackMeta) -> Self {
        Self {
            cluster_track_uuid: generate_cluster_track_uuid(room_id, peer_id, track_name),
            track_id,
            track_name: track_name.into(),
            track_meta,
            out_actions: Default::default(),
            active: false, //we need wait for first rtp packet
        }
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
            active: self.active,
            label: None,
        }
    }

    pub fn on_tick(&mut self, _now_ms: u64) {}

    pub fn on_cluster_event(&mut self, event: ClusterRemoteTrackIncomingEvent) {
        match event {
            ClusterRemoteTrackIncomingEvent::RequestKeyFrame => {
                log::info!("[RemoteTrack {}] request keyframe", self.track_name);
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
                log::debug!("[RemoteTrack {}] media from transport pkt {:?} {}", self.track_name, pkt.codec, pkt.seq_no);
                if !self.active {
                    self.active = true;
                    self.out_actions
                        .push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackAdded(self.track_name.clone(), self.cluster_meta())));
                }
                self.out_actions.push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt)));
            }
            RemoteTrackIncomingEvent::Rpc(event) => match event {
                RemoteTrackRpcIn::Toggle(req) => {
                    if req.data.track.is_none() {
                        self.active = false;
                        self.out_actions
                            .push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackRemoved(self.track_name.clone())));
                    }

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
    pub fn close(&mut self) {
        if self.active {
            self.out_actions
                .push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackRemoved(self.track_name.clone())));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        endpoint_wrap::internal::remote_track::RemoteTrackOutput,
        rpc::{RemoteTrackRpcIn, RemoteTrackRpcOut, SenderToggle},
        RpcRequest, RpcResponse,
    };
    use cluster::{ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent};
    use transport::{MediaKind, MediaPacket, RemoteTrackOutgoingEvent, TrackMeta};

    use super::RemoteTrack;

    #[test]
    fn normal_cluster_events() {
        let mut track = RemoteTrack::new("room1", "peer1", 100, "audio_main", TrackMeta::new_audio(None));

        let pkt = MediaPacket::default_audio(1, 1000, vec![1, 2, 3]);
        track.on_transport_event(transport::RemoteTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackAdded("audio_main".to_string(), track.cluster_meta())))
        );
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt))));
        assert_eq!(track.pop_action(), None);

        let pkt = MediaPacket::default_audio(2, 1000, vec![1, 2, 3]);
        track.on_transport_event(transport::RemoteTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt))));
        assert_eq!(track.pop_action(), None);

        // toggle off
        track.on_transport_event(transport::RemoteTrackIncomingEvent::Rpc(RemoteTrackRpcIn::Toggle(RpcRequest::from(
            1,
            SenderToggle {
                name: "audio_main".to_string(),
                kind: MediaKind::Audio,
                track: None,
                label: None,
            },
        ))));
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackRemoved("audio_main".to_string())))
        );
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Transport(RemoteTrackOutgoingEvent::Rpc(RemoteTrackRpcOut::ToggleRes(RpcResponse::success(1, true)))))
        );
        assert_eq!(track.pop_action(), None);

        // toggle on
        track.on_transport_event(transport::RemoteTrackIncomingEvent::Rpc(RemoteTrackRpcIn::Toggle(RpcRequest::from(
            2,
            SenderToggle {
                name: "audio_main".to_string(),
                kind: MediaKind::Audio,
                track: Some("remote_track_id".to_string()),
                label: None,
            },
        ))));

        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Transport(RemoteTrackOutgoingEvent::Rpc(RemoteTrackRpcOut::ToggleRes(RpcResponse::success(2, true)))))
        );
        assert_eq!(track.pop_action(), None);

        // reactive with incoming pkt
        let pkt = MediaPacket::default_audio(3, 1000, vec![1, 2, 3]);
        track.on_transport_event(transport::RemoteTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackAdded("audio_main".to_string(), track.cluster_meta())))
        );
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt))));
        assert_eq!(track.pop_action(), None);

        track.close();
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackRemoved("audio_main".to_string())))
        );
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
