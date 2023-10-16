use std::collections::VecDeque;

use cluster::{ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent};
use transport::{LocalTrackIncomingEvent, LocalTrackOutgoingEvent, TrackId, TrackMeta};
use utils::hash_str;

use crate::{
    rpc::{BitrateLimit, LocalTrackRpcIn, LocalTrackRpcOut},
    RpcResponse,
};

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
}

impl LocalTrack {
    pub fn new(room_id: &str, peer_id: &str, track_id: TrackId, track_name: &str, track_meta: TrackMeta) -> Self {
        Self {
            room_id: room_id.into(),
            peer_id: peer_id.into(),
            track_id,
            track_name: track_name.into(),
            track_meta,
            source: None,
            out_actions: Default::default(),
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {}

    pub fn on_cluster_event(&mut self, event: ClusterLocalTrackIncomingEvent) {
        match event {
            ClusterLocalTrackIncomingEvent::MediaPacket(pkt) => {
                log::debug!("[LocalTrack {}] media from cluster pkt {} {}", self.track_name, pkt.pt, pkt.seq_no);
                self.out_actions.push_back(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::MediaPacket(pkt)));
            }
        }
    }

    pub fn on_transport_event(&mut self, event: LocalTrackIncomingEvent<LocalTrackRpcIn>) {
        match event {
            LocalTrackIncomingEvent::RequestKeyFrame => {
                self.out_actions.push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::RequestKeyFrame));
            }
            LocalTrackIncomingEvent::Rpc(rpc) => match rpc {
                LocalTrackRpcIn::Switch(req) => {
                    let old_source = self.source.replace(LocalTrackSource::new(&req.data.remote.peer, &req.data.remote.stream));
                    if let Some(old_source) = old_source {
                        self.out_actions
                            .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe(old_source.peer, old_source.track)));
                    }

                    self.out_actions
                        .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Subscribe(req.data.remote.peer, req.data.remote.stream)));
                    self.out_actions
                        .push_back(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(
                            req.req_id, true,
                        )))));
                }
                LocalTrackRpcIn::Limit(req) => {
                    //TODO
                    self.out_actions
                        .push_back(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::LimitRes(RpcResponse::success(
                            req.req_id, true,
                        )))));
                }
                LocalTrackRpcIn::Disconnect(req) => {
                    if let Some(old_source) = self.source.take() {
                        self.out_actions
                            .push_back(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe(old_source.peer, old_source.track)));
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
    use transport::{LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaKind, MediaPacket, MediaPacketExtensions, MediaSampleRate, TrackMeta};

    use crate::{
        endpoint_wrap::internal::local_track::LocalTrackOutput,
        rpc::{LocalTrackRpcIn, LocalTrackRpcOut, ReceiverDisconnect, ReceiverSwitch, RemoteStream},
        RpcRequest, RpcResponse,
    };

    use super::LocalTrack;

    #[test]
    fn incoming_cluster_media_should_fire_transport() {
        let mut track = LocalTrack::new(
            "room1",
            "peer1",
            100,
            "audio_main",
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        );

        let pkt = MediaPacket {
            pt: 111,
            seq_no: 1,
            time: 1000,
            marker: true,
            ext_vals: MediaPacketExtensions {
                abs_send_time: None,
                transport_cc: None,
            },
            nackable: true,
            payload: vec![1, 2, 3],
        };
        track.on_cluster_event(ClusterLocalTrackIncomingEvent::MediaPacket(pkt.clone()));
        assert_eq!(track.pop_action(), Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::MediaPacket(pkt))));
    }

    #[test]
    fn incoming_transport_keyframe_request_should_fire_cluster() {
        let mut track = LocalTrack::new(
            "room1",
            "peer1",
            100,
            "audio_main",
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        );

        track.on_transport_event(LocalTrackIncomingEvent::RequestKeyFrame);
        assert_eq!(track.pop_action(), Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::RequestKeyFrame)));
    }

    #[test]
    fn incoming_rpc_switch_disconnect() {
        let mut track = LocalTrack::new(
            "room1",
            "peer1",
            100,
            "audio_main",
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        );

        track.on_transport_event(LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
            req_id: 1,
            data: ReceiverSwitch {
                id: "audio_0".to_string(),
                priority: 100,
                remote: RemoteStream {
                    peer: "peer2".into(),
                    stream: "audio_main".into(),
                },
            },
        })));

        // should output cluster subscribe and transport switch res
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Subscribe("peer2".into(), "audio_main".into())))
        );
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(1, true)))))
        );
        assert_eq!(track.pop_action(), None);

        // now we switch to other peer
        track.on_transport_event(LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
            req_id: 2,
            data: ReceiverSwitch {
                id: "audio_0".to_string(),
                priority: 100,
                remote: RemoteStream {
                    peer: "peer3".into(),
                    stream: "audio_main".into(),
                },
            },
        })));

        // should output cluster unsubscribe and cluster subscribe and transport switch res
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe("peer2".into(), "audio_main".into())))
        );
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Subscribe("peer3".into(), "audio_main".into())))
        );
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(2, true)))))
        );
        assert_eq!(track.pop_action(), None);

        // now we disconnect
        track.on_transport_event(LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Disconnect(RpcRequest {
            req_id: 3,
            data: ReceiverDisconnect { id: "audio_0".to_string() },
        })));

        // should output cluster unsubscribe and transport disconnect res
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Cluster(ClusterLocalTrackOutgoingEvent::Unsubscribe("peer3".into(), "audio_main".into())))
        );
        assert_eq!(
            track.pop_action(),
            Some(LocalTrackOutput::Transport(LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::DisconnectRes(RpcResponse::success(
                3, true
            )))))
        );
        assert_eq!(track.pop_action(), None);
    }
}
