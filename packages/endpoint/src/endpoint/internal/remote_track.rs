use cluster::{ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent, ClusterTrackMeta, ClusterTrackScalingType, ClusterTrackStats, ClusterTrackStatus, ClusterTrackUuid};
use std::collections::VecDeque;
use transport::{RemoteTrackIncomingEvent, RemoteTrackOutgoingEvent, TrackId, TrackMeta};

const BITRATE_WINDOW_MS: u64 = 2_000;

use crate::{
    rpc::{RemoteTrackRpcIn, RemoteTrackRpcOut},
    RpcResponse,
};

use self::bitrate_measure::BitrateMeasure;

mod bitrate_measure;

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
    bitrate_measure: Option<(BitrateMeasure, Option<ClusterTrackStats>)>,
    consumers_limit: Option<u32>,
}

impl RemoteTrack {
    pub fn new(room_id: &str, peer_id: &str, track_id: TrackId, track_name: &str, track_meta: TrackMeta) -> Self {
        Self {
            cluster_track_uuid: ClusterTrackUuid::from_info(room_id, peer_id, track_name),
            track_id,
            track_name: track_name.into(),
            bitrate_measure: if track_meta.kind.is_video() {
                Some((BitrateMeasure::new(BITRATE_WINDOW_MS), None))
            } else {
                None
            },
            track_meta,
            out_actions: Default::default(),
            active: false, //we need wait for first rtp packet
            consumers_limit: None,
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

    pub fn consumers_limit(&self) -> Option<u32> {
        self.consumers_limit
    }

    pub fn on_tick(&mut self, _now_ms: u64) {}

    pub fn on_cluster_event(&mut self, event: ClusterRemoteTrackIncomingEvent) {
        match event {
            ClusterRemoteTrackIncomingEvent::RequestKeyFrame(kind) => {
                log::info!("[RemoteTrack {}] request keyframe", self.track_name);
                self.out_actions.push_back(RemoteTrackOutput::Transport(RemoteTrackOutgoingEvent::RequestKeyFrame(kind)));
            }
            ClusterRemoteTrackIncomingEvent::RequestLimitBitrate(bitrate) => {
                log::debug!("[RemoteTrack {}] request limit bitrate {bitrate}", self.track_name);
                if let Some((_bitrate_measure, Some(previous_stats))) = &self.bitrate_measure {
                    self.consumers_limit = Some((bitrate as f32 * previous_stats.consumer_bitrate_scale()) as u32);
                } else {
                    self.consumers_limit = Some(bitrate); //default 1x
                }
            }
        }
    }

    pub fn on_transport_event(&mut self, now_ms: u64, event: RemoteTrackIncomingEvent<RemoteTrackRpcIn>) {
        match event {
            RemoteTrackIncomingEvent::MediaPacket(pkt) => {
                log::debug!("[RemoteTrack {}] media from transport pkt {:?} {}", self.track_name, pkt.codec, pkt.seq_no);
                if !self.active {
                    self.active = true;
                    self.out_actions
                        .push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackAdded(self.track_name.clone(), self.cluster_meta())));
                }
                if let Some((bitrate_measure, previous_stats)) = &mut self.bitrate_measure {
                    if let Some(stats) = bitrate_measure.add_sample(now_ms, &pkt.codec, pkt.payload.len()) {
                        log::debug!("[RemoteTrack {}] stats {:?}", self.track_name, stats);
                        *previous_stats = Some(stats.clone());
                        self.out_actions.push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackStats(stats)));
                    } else if pkt.codec.is_key() {
                        if let Some(stats) = previous_stats {
                            self.out_actions.push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackStats(stats.clone())));
                        }
                    }
                }
                self.out_actions.push_back(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackMedia(pkt)));
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
        endpoint::internal::remote_track::RemoteTrackOutput,
        rpc::{RemoteTrackRpcIn, RemoteTrackRpcOut, SenderToggle},
        RpcRequest, RpcResponse,
    };
    use cluster::{ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent};
    use transport::{MediaKind, MediaPacket, RemoteTrackOutgoingEvent, RequestKeyframeKind, TrackMeta};

    use super::RemoteTrack;

    #[test]
    fn normal_cluster_events() {
        let mut track = RemoteTrack::new("room1", "peer1", 100, "audio_main", TrackMeta::new_audio(None));

        let pkt = MediaPacket::simple_audio(1, 1000, vec![1, 2, 3]);
        track.on_transport_event(0, transport::RemoteTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackAdded("audio_main".to_string(), track.cluster_meta())))
        );
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackMedia(pkt))));
        assert_eq!(track.pop_action(), None);

        let pkt = MediaPacket::simple_audio(2, 1000, vec![1, 2, 3]);
        track.on_transport_event(0, transport::RemoteTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackMedia(pkt))));
        assert_eq!(track.pop_action(), None);

        // toggle off
        track.on_transport_event(
            0,
            transport::RemoteTrackIncomingEvent::Rpc(RemoteTrackRpcIn::Toggle(RpcRequest::from(
                1,
                SenderToggle {
                    name: "audio_main".to_string(),
                    kind: MediaKind::Audio,
                    track: None,
                    label: None,
                },
            ))),
        );
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
        track.on_transport_event(
            0,
            transport::RemoteTrackIncomingEvent::Rpc(RemoteTrackRpcIn::Toggle(RpcRequest::from(
                2,
                SenderToggle {
                    name: "audio_main".to_string(),
                    kind: MediaKind::Audio,
                    track: Some("remote_track_id".to_string()),
                    label: None,
                },
            ))),
        );

        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Transport(RemoteTrackOutgoingEvent::Rpc(RemoteTrackRpcOut::ToggleRes(RpcResponse::success(2, true)))))
        );
        assert_eq!(track.pop_action(), None);

        // reactive with incoming pkt
        let pkt = MediaPacket::simple_audio(3, 1000, vec![1, 2, 3]);
        track.on_transport_event(0, transport::RemoteTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackAdded("audio_main".to_string(), track.cluster_meta())))
        );
        assert_eq!(track.pop_action(), Some(RemoteTrackOutput::Cluster(ClusterRemoteTrackOutgoingEvent::TrackMedia(pkt))));
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

        track.on_cluster_event(ClusterRemoteTrackIncomingEvent::RequestKeyFrame(RequestKeyframeKind::Pli));
        assert_eq!(
            track.pop_action(),
            Some(RemoteTrackOutput::Transport(transport::RemoteTrackOutgoingEvent::RequestKeyFrame(RequestKeyframeKind::Pli)))
        );
        assert_eq!(track.pop_action(), None);
    }

    #[test]
    fn incoming_pkt_should_fire_stats() {
        //TODO test calc bitrate
    }

    #[test]
    fn incoming_video_key_frame_should_fire_stats_before() {
        //TODO test incoming_video_key_frame_should_fire_stats_before
    }
}
