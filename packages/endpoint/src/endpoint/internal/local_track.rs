use std::collections::VecDeque;

use cluster::{ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent};
use transport::{LocalTrackIncomingEvent, LocalTrackOutgoingEvent, RequestKeyframeKind, TrackId, TrackMeta};

use crate::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut},
    RpcResponse,
};

use self::{scalable_filter::ScalablePacketFilter, source_stats_gen::SourceStatsGenerator};

use super::{bitrate_allocator::LocalTrackTarget, MediaEndpointInternalLocalTrackControl};

mod scalable_filter;
mod source_stats_gen;

#[derive(PartialEq, Eq, Clone)]
pub struct LocalTrackSource {
    pub(crate) peer: String,
    pub(crate) track: String,
}

impl LocalTrackSource {
    pub fn new(peer: &str, track: &str) -> Self {
        Self {
            peer: peer.into(),
            track: track.into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LocalTrackOutput {
    Control(MediaEndpointInternalLocalTrackControl),
    Transport(LocalTrackOutgoingEvent<LocalTrackRpcOut>),
    Cluster(ClusterLocalTrackOutgoingEvent),
}

#[allow(dead_code)]
pub struct LocalTrack {
    room_id: String,
    peer_id: String,
    track_id: TrackId,
    track_name: String,
    track_meta: TrackMeta,
    source: Option<LocalTrackSource>,
    out_actions: VecDeque<LocalTrackOutput>,
    filter: ScalablePacketFilter,
    source_stats_generator: SourceStatsGenerator,
}

impl LocalTrack {
    pub fn new(room_id: &str, peer_id: &str, track_id: TrackId, track_name: &str, track_meta: TrackMeta) -> Self {
        let sample_rate: u32 = track_meta.sample_rate.clone().into();
        Self {
            room_id: room_id.into(),
            peer_id: peer_id.into(),
            track_id,
            track_name: track_name.into(),
            track_meta,
            source: None,
            out_actions: Default::default(),
            filter: ScalablePacketFilter::new(sample_rate),
            source_stats_generator: Default::default(),
        }
    }

    pub fn set_target(&mut self, target: LocalTrackTarget) {
        log::info!("[LocalTrack {}] set target {:?}", self.track_name, target);
        if self.filter.set_target(target) {
            log::info!("[LocalTrack {}] request key-frame", self.track_name);
            self.out_actions
                .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::RequestKeyFrame(RequestKeyframeKind::Pli)));
        }
    }

    pub fn set_bitrate(&mut self, bitrate: u32) {
        log::debug!("[LocalTrack {}] set bitrate {:?}", self.track_name, bitrate);
        self.out_actions.push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::LimitBitrate(bitrate)));
    }

    pub fn on_tick(&mut self, _now_ms: u64) {}

    pub fn on_cluster_event(&mut self, now_ms: u64, event: ClusterLocalTrackIncomingEvent) {
        match event {
            ClusterLocalTrackIncomingEvent::MediaPacket(_track_uuid, pkt) => {
                if let Some(stats) = self.source_stats_generator.on_pkt(&pkt) {
                    log::info!("[LocalTrack {}] auto generate stats {:?}", self.track_name, stats);
                    self.out_actions.push_back(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::SourceStats { stats }));
                }

                if let Some(pkt) = self.filter.process(now_ms, pkt) {
                    log::debug!("[LocalTrack {}] media from cluster pkt {:?} {}", self.track_name, pkt.codec, pkt.seq_no);
                    self.out_actions.push_back(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::MediaPacket(pkt)));
                }
            }
            ClusterLocalTrackIncomingEvent::MediaStats(_track_uuid, stats) => {
                log::info!("[LocalTrack {}] stats {:?}", self.track_name, stats);
                self.out_actions.push_back(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::SourceStats { stats }));
                self.source_stats_generator.arrived_stats();
            }
        }
    }

    pub fn on_transport_event(&mut self, event: LocalTrackIncomingEvent<LocalTrackRpcIn>) {
        match event {
            LocalTrackIncomingEvent::RequestKeyFrame(kind) => {
                self.out_actions.push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::RequestKeyFrame(kind)));
            }
            LocalTrackIncomingEvent::Rpc(rpc) => match rpc {
                LocalTrackRpcIn::Switch(req) => {
                    let new_source = LocalTrackSource::new(&req.data.remote.peer, &req.data.remote.stream);
                    // only switch if the source is different
                    if !self.source.eq(&Some(new_source.clone())) {
                        self.source_stats_generator.switched_source();
                        self.filter.switched_source();
                        let old_source = self.source.replace(new_source);
                        if let Some(old_source) = old_source {
                            self.out_actions
                                .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe(old_source.peer, old_source.track)));
                        }
                        self.out_actions
                            .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Subscribe(req.data.remote.peer, req.data.remote.stream)));
                    }

                    if self.track_meta.kind.is_video() {
                        self.out_actions
                            .push_back(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::SourceSet { priority: req.data.priority }));
                    }
                    self.out_actions
                        .push_back(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(
                            req.req_id, true,
                        )))));
                }
                LocalTrackRpcIn::Limit(req) => {
                    self.out_actions
                        .push_back(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::Limit { limit: req.data.limit }));
                    self.out_actions
                        .push_back(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::LimitRes(RpcResponse::success(
                            req.req_id, true,
                        )))));
                }
                LocalTrackRpcIn::Disconnect(req) => {
                    self.source_stats_generator.switched_source();
                    self.filter.switched_source();
                    if let Some(old_source) = self.source.take() {
                        self.out_actions
                            .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe(old_source.peer, old_source.track)));
                        if self.track_meta.kind.is_video() {
                            self.out_actions.push_back(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::SourceRemove));
                        }
                    }
                    self.out_actions
                        .push_back(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::DisconnectRes(RpcResponse::success(
                            req.req_id, true,
                        )))));
                }
            },
        }
    }

    pub fn pop_action(&mut self) -> Option<LocalTrackOutput> {
        self.out_actions.pop_front()
    }

    /// Close the track and cleanup everything
    /// This should be called when the track is removed from the peer
    /// - Unsubscribe from cluster if need
    pub fn close(&mut self) {
        if let Some(old_source) = self.source.take() {
            self.out_actions
                .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe(old_source.peer, old_source.track)));
        }
    }
}

#[cfg(test)]
mod tests {
    use cluster::{ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent};
    use transport::{LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, RequestKeyframeKind, TrackMeta};

    use crate::{
        endpoint::internal::{local_track::LocalTrackOutput, MediaEndpointInternalLocalTrackControl},
        rpc::{LocalTrackRpcIn, LocalTrackRpcOut, ReceiverDisconnect, ReceiverSwitch, RemoteStream},
        RpcRequest, RpcResponse,
    };

    use super::LocalTrack;

    #[test]
    fn incoming_cluster_media_should_fire_transport() {
        let mut track = LocalTrack::new("room1", "peer1", 100, "audio_main", TrackMeta::new_audio(None));

        let pkt = MediaPacket::simple_audio(1, 0, vec![1, 2, 3]);
        track.on_cluster_event(0, ClusterLocalTrackIncomingEvent::MediaPacket(0.into(), pkt.clone()));
        assert_eq!(track.pop_action(), Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::MediaPacket(pkt))));
    }

    #[test]
    fn incoming_transport_keyframe_request_should_fire_cluster() {
        let mut track = LocalTrack::new("room1", "peer1", 100, "audio_main", TrackMeta::new_audio(None));

        track.on_transport_event(LocalTrackIncomingEvent::RequestKeyFrame(RequestKeyframeKind::Pli));
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::RequestKeyFrame(RequestKeyframeKind::Pli)))
        );
    }

    #[test]
    fn incoming_rpc_switch_disconnect() {
        let mut track = LocalTrack::new("room1", "peer1", 100, "video_main", TrackMeta::new_video(None));
        let priority = 100;

        track.on_transport_event(LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
            req_id: 1,
            data: ReceiverSwitch {
                id: "video_0".to_string(),
                priority,
                remote: RemoteStream {
                    peer: "peer2".into(),
                    stream: "video_main".into(),
                },
            },
        })));

        // should output cluster subscribe and transport switch res
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Subscribe("peer2".into(), "video_main".into())))
        );
        assert_eq!(track.pop_action(), Some(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::SourceSet { priority })));
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(1, true)))))
        );
        assert_eq!(track.pop_action(), None);

        // now we switch to other peer
        track.on_transport_event(LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
            req_id: 2,
            data: ReceiverSwitch {
                id: "video_0".to_string(),
                priority,
                remote: RemoteStream {
                    peer: "peer3".into(),
                    stream: "video_main".into(),
                },
            },
        })));

        // should output cluster unsubscribe and cluster subscribe and transport switch res
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe("peer2".into(), "video_main".into())))
        );
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Subscribe("peer3".into(), "video_main".into())))
        );
        assert_eq!(track.pop_action(), Some(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::SourceSet { priority })));
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(2, true)))))
        );
        assert_eq!(track.pop_action(), None);

        // now we disconnect
        track.on_transport_event(LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Disconnect(RpcRequest {
            req_id: 3,
            data: ReceiverDisconnect { id: "video_0".to_string() },
        })));

        // should output cluster unsubscribe and transport disconnect res
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe("peer3".into(), "video_main".into())))
        );
        assert_eq!(track.pop_action(), Some(LocalTrackOutput::Control(MediaEndpointInternalLocalTrackControl::SourceRemove)));
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::DisconnectRes(RpcResponse::success(
                3, true
            )))))
        );
        assert_eq!(track.pop_action(), None);
    }

    //TODO testing auto generate source stats from first media packet if not arrived any stats before that
}
