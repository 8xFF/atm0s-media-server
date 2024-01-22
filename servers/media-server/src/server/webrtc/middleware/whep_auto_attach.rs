use std::collections::VecDeque;

use cluster::{ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent};
use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn, MediaEndpointInternalControl, MediaEndpointInternalLocalTrackControl, MediaEndpointMiddleware, MediaEndpointMiddlewareOutput,
};
use transport::{MediaKind, TrackId, TransportError, TransportIncomingEvent};

const WHEP_LOCAL_VIDEO_TRACK_ID: TrackId = 1;
const WHEP_LOCAL_VIDEO_TRACK_PRIORITY: u16 = 1000;

#[derive(Default)]
pub struct WhepAutoAttachMediaTrackMiddleware {
    actions: VecDeque<MediaEndpointMiddlewareOutput>,
    bind_video: Option<(String, String)>,
    source_queue: VecDeque<(String, String)>,
}

impl WhepAutoAttachMediaTrackMiddleware {
    fn select_video(&mut self) {
        if let Some((peer, track)) = self.source_queue.pop_front() {
            self.bind_video.replace((peer.clone(), track.clone()));
            self.actions.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                WHEP_LOCAL_VIDEO_TRACK_ID,
                cluster::ClusterLocalTrackOutgoingEvent::Subscribe(peer, track),
            )));
            self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::LocalTrack(
                WHEP_LOCAL_VIDEO_TRACK_ID,
                MediaEndpointInternalLocalTrackControl::SourceSet {
                    priority: WHEP_LOCAL_VIDEO_TRACK_PRIORITY,
                },
            )));
        }
    }
}

impl MediaEndpointMiddleware for WhepAutoAttachMediaTrackMiddleware {
    fn on_start(&mut self, _now_ms: u64) {}

    fn on_tick(&mut self, _now_ms: u64) {}

    fn on_transport(&mut self, _now_ms: u64, _event: &TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>) -> bool {
        false
    }

    fn on_transport_error(&mut self, _now_ms: u64, _error: &TransportError) -> bool {
        false
    }

    fn on_cluster(&mut self, _now_ms: u64, event: &ClusterEndpointIncomingEvent) -> bool {
        match event {
            ClusterEndpointIncomingEvent::PeerTrackAdded(peer, track, meta) => {
                if meta.kind != MediaKind::Video {
                    return false;
                }

                self.source_queue.push_back((peer.to_string(), track.to_string()));

                if self.bind_video.is_none() {
                    self.select_video();
                }
            }
            ClusterEndpointIncomingEvent::PeerTrackRemoved(peer, track) => {
                //remove from source queue
                if let Some(index) = self.source_queue.iter().position(|(p, t)| p.eq(peer) && t.eq(track)) {
                    self.source_queue.remove(index);
                }

                if let Some((bind_peer, bind_track)) = &self.bind_video {
                    if bind_peer.eq(peer) && bind_track.eq(track) {
                        self.bind_video.take();
                        self.actions.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                            WHEP_LOCAL_VIDEO_TRACK_ID,
                            cluster::ClusterLocalTrackOutgoingEvent::Unsubscribe(peer.to_string(), track.to_string()),
                        )));
                        self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::LocalTrack(
                            WHEP_LOCAL_VIDEO_TRACK_ID,
                            MediaEndpointInternalLocalTrackControl::SourceRemove,
                        )));
                        self.select_video();
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn pop_action(&mut self, _now_ms: u64) -> Option<MediaEndpointMiddlewareOutput> {
        self.actions.pop_front()
    }

    fn before_drop(&mut self, _now_ms: u64) {
        if let Some((peer, track)) = self.bind_video.take() {
            self.actions.push_back(MediaEndpointMiddlewareOutput::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                WHEP_LOCAL_VIDEO_TRACK_ID,
                cluster::ClusterLocalTrackOutgoingEvent::Unsubscribe(peer, track),
            )));
            self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::LocalTrack(
                WHEP_LOCAL_VIDEO_TRACK_ID,
                MediaEndpointInternalLocalTrackControl::SourceRemove,
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use cluster::ClusterTrackMeta;
    use endpoint::MediaEndpointMiddleware;

    #[test]
    fn switch_first_video() {
        let mut middleware = super::WhepAutoAttachMediaTrackMiddleware::default();

        middleware.on_cluster(
            0,
            &cluster::ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), ClusterTrackMeta::default_video()),
        );

        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::LocalTrackEvent(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                cluster::ClusterLocalTrackOutgoingEvent::Subscribe("peer1".to_string(), "track1".to_string()),
            )))
        );

        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Control(super::MediaEndpointInternalControl::LocalTrack(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                super::MediaEndpointInternalLocalTrackControl::SourceSet {
                    priority: super::WHEP_LOCAL_VIDEO_TRACK_PRIORITY,
                },
            )))
        );
        assert_eq!(middleware.pop_action(0), None);

        // now unsub with before_drop
        middleware.before_drop(0);
        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::LocalTrackEvent(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                cluster::ClusterLocalTrackOutgoingEvent::Unsubscribe("peer1".to_string(), "track1".to_string()),
            )))
        );
        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Control(super::MediaEndpointInternalControl::LocalTrack(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                super::MediaEndpointInternalLocalTrackControl::SourceRemove,
            )))
        );
        assert_eq!(middleware.pop_action(0), None);
    }

    #[test]
    fn fallback_to_next_video() {
        let mut middleware = super::WhepAutoAttachMediaTrackMiddleware::default();

        middleware.on_cluster(
            0,
            &cluster::ClusterEndpointIncomingEvent::PeerTrackAdded("peer1".to_string(), "track1".to_string(), ClusterTrackMeta::default_video()),
        );

        middleware.on_cluster(
            0,
            &cluster::ClusterEndpointIncomingEvent::PeerTrackAdded("peer2".to_string(), "track2".to_string(), ClusterTrackMeta::default_video()),
        );

        // will sub first video
        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::LocalTrackEvent(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                cluster::ClusterLocalTrackOutgoingEvent::Subscribe("peer1".to_string(), "track1".to_string()),
            )))
        );

        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Control(super::MediaEndpointInternalControl::LocalTrack(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                super::MediaEndpointInternalLocalTrackControl::SourceSet {
                    priority: super::WHEP_LOCAL_VIDEO_TRACK_PRIORITY,
                },
            )))
        );

        assert_eq!(middleware.pop_action(0), None);

        // if first video removed, will unsub first video then sub second video
        middleware.on_cluster(0, &cluster::ClusterEndpointIncomingEvent::PeerTrackRemoved("peer1".to_string(), "track1".to_string()));

        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::LocalTrackEvent(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                cluster::ClusterLocalTrackOutgoingEvent::Unsubscribe("peer1".to_string(), "track1".to_string()),
            )))
        );
        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Control(super::MediaEndpointInternalControl::LocalTrack(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                super::MediaEndpointInternalLocalTrackControl::SourceRemove,
            )))
        );

        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::LocalTrackEvent(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                cluster::ClusterLocalTrackOutgoingEvent::Subscribe("peer2".to_string(), "track2".to_string()),
            )))
        );

        assert_eq!(
            middleware.pop_action(0),
            Some(super::MediaEndpointMiddlewareOutput::Control(super::MediaEndpointInternalControl::LocalTrack(
                super::WHEP_LOCAL_VIDEO_TRACK_ID,
                super::MediaEndpointInternalLocalTrackControl::SourceSet {
                    priority: super::WHEP_LOCAL_VIDEO_TRACK_PRIORITY,
                },
            )))
        );
    }
}
