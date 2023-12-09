use std::collections::{HashMap, VecDeque};

use cluster::{ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent};
use media_utils::hash_str;
use transport::{MediaKind, TrackId, TransportIncomingEvent, TransportOutgoingEvent, TransportStateEvent};

use crate::rpc::{EndpointRpcIn, EndpointRpcOut, LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut, TrackInfo};

use self::{
    bitrate_allocator::BitrateAllocationAction,
    local_track::{LocalTrack, LocalTrackInternalOutputEvent, LocalTrackOutput},
    remote_track::{RemoteTrack, RemoteTrackOutput},
};

const DEFAULT_BITRATE_OUT_BPS: u32 = 3_000_000; //3Mbps
const MAX_BITRATE_IN_BPS: u32 = 3_000_000; //3Mbps

mod bitrate_allocator;
mod bitrate_limiter;
mod local_track;
mod remote_track;

pub use bitrate_limiter::BitrateLimiterType;

#[derive(Debug, PartialEq, Eq)]
pub enum MediaEndpointInternalEvent {
    ConnectionClosed,
    ConnectionCloseRequest,
    SubscribePeer(String),
    UnsubscribePeer(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum MediaInternalAction {
    Internal(MediaEndpointInternalEvent),
    Endpoint(TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>),
    Cluster(ClusterEndpointOutgoingEvent),
}

pub struct MediaEndpointInternal {
    room_id: String,
    peer_id: String,
    cluster_track_map: HashMap<(String, String), MediaKind>,
    local_track_map: HashMap<String, TrackId>,
    output_actions: VecDeque<MediaInternalAction>,
    local_tracks: HashMap<TrackId, LocalTrack>,
    remote_tracks: HashMap<TrackId, RemoteTrack>,
    bitrate_allocator: bitrate_allocator::BitrateAllocator,
    bitrate_limiter: bitrate_limiter::BitrateLimiter,
}

impl MediaEndpointInternal {
    pub fn new(room_id: &str, peer_id: &str, bitrate_limiter: BitrateLimiterType) -> Self {
        log::info!("[MediaEndpointInternal {}/{}] create", room_id, peer_id);
        Self {
            room_id: room_id.into(),
            peer_id: peer_id.into(),
            cluster_track_map: HashMap::new(),
            local_track_map: HashMap::new(),
            output_actions: VecDeque::with_capacity(100),
            local_tracks: HashMap::new(),
            remote_tracks: HashMap::new(),
            bitrate_allocator: bitrate_allocator::BitrateAllocator::new(DEFAULT_BITRATE_OUT_BPS),
            bitrate_limiter: bitrate_limiter::BitrateLimiter::new(bitrate_limiter, MAX_BITRATE_IN_BPS),
        }
    }

    fn push_rpc(&mut self, rpc: EndpointRpcOut) {
        self.output_actions.push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::Rpc(rpc)));
    }

    fn push_cluster(&mut self, event: ClusterEndpointOutgoingEvent) {
        self.output_actions.push_back(MediaInternalAction::Cluster(event));
    }

    fn push_internal(&mut self, event: MediaEndpointInternalEvent) {
        self.output_actions.push_back(MediaInternalAction::Internal(event));
    }

    fn pop_internal(&mut self, now_ms: u64) {
        loop {
            if self.pop_tracks_actions(now_ms) {
                continue;
            }
            if self.pop_bitrate_allocation_action(now_ms) {
                continue;
            }
            break;
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        for (_, track) in self.local_tracks.iter_mut() {
            track.on_tick(now_ms);
        }

        if !self.remote_tracks.is_empty() {
            self.bitrate_limiter.reset();
            for (_, track) in self.remote_tracks.iter_mut() {
                track.on_tick(now_ms);
                self.bitrate_limiter.add_remote(track.consumers_limit());
            }

            self.output_actions
                .push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::LimitIngressBitrate(self.bitrate_limiter.final_bitrate())));
        }

        self.bitrate_allocator.tick();
        self.pop_internal(now_ms);
    }

    pub fn on_transport(&mut self, now_ms: u64, event: TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>) {
        match event {
            TransportIncomingEvent::EgressBitrateEstimate(bitrate) => {
                self.bitrate_allocator.set_est_bitrate(bitrate as u32);
                self.pop_bitrate_allocation_action(now_ms);
            }
            TransportIncomingEvent::State(state) => {
                log::info!("[EndpointInternal] switch state to {:?}", state);
                match state {
                    TransportStateEvent::Connected => {}
                    TransportStateEvent::Reconnecting => {}
                    TransportStateEvent::Reconnected => {}
                    TransportStateEvent::Disconnected => {
                        self.push_internal(MediaEndpointInternalEvent::ConnectionClosed);
                    }
                }
            }
            TransportIncomingEvent::Continue => {}
            TransportIncomingEvent::Rpc(rpc) => self.process_rpc(rpc),
            TransportIncomingEvent::Stats(_stats) => {
                //TODO process stats event for limiting local tracks
            }
            TransportIncomingEvent::RemoteTrackAdded(track_name, track_id, meta) => {
                log::info!("[EndpointInternal] on remote track added {} {}", track_name, track_id);
                if !self.remote_tracks.contains_key(&track_id) {
                    let track = RemoteTrack::new(&self.room_id, &self.peer_id, track_id, &track_name, meta);
                    self.remote_tracks.insert(track_id, track);
                } else {
                    log::warn!("[EndpointInternal] remote track already exists {:?}", track_id);
                }
            }
            TransportIncomingEvent::RemoteTrackEvent(track_id, event) => {
                if let Some(track) = self.remote_tracks.get_mut(&track_id) {
                    track.on_transport_event(now_ms, event);
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?} for event", track_id);
                }
            }
            TransportIncomingEvent::RemoteTrackRemoved(_track_name, track_id) => {
                if let Some(mut track) = self.remote_tracks.remove(&track_id) {
                    track.close();
                    self.pop_remote_track_actions(track_id, &mut track);
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?} for removed", track_id);
                }
            }
            TransportIncomingEvent::LocalTrackAdded(track_name, track_id, meta) => {
                if !self.local_tracks.contains_key(&track_id) {
                    let track = LocalTrack::new(&self.room_id, &self.peer_id, track_id, &track_name, meta);
                    self.local_tracks.insert(track_id, track);
                    self.local_track_map.insert(track_name, track_id);
                } else {
                    log::warn!("[EndpointInternal] local track already exists {:?}", track_id);
                }
            }
            TransportIncomingEvent::LocalTrackEvent(track_id, event) => {
                if let Some(track) = self.local_tracks.get_mut(&track_id) {
                    track.on_transport_event(event);
                } else {
                    log::warn!("[EndpointInternal] local track not found {:?}", track_id);
                }
            }
            TransportIncomingEvent::LocalTrackRemoved(track_name, track_id) => {
                if let Some(mut track) = self.local_tracks.remove(&track_id) {
                    track.close();
                    self.pop_local_track_actions(now_ms, track_id, &mut track);
                    self.local_track_map.remove(&track_name);
                } else {
                    log::warn!("[EndpointInternal] local track not found {:?}", track_id);
                }
            }
        }

        self.pop_internal(now_ms);
    }

    pub fn on_cluster(&mut self, now_ms: u64, event: ClusterEndpointIncomingEvent) {
        match event {
            ClusterEndpointIncomingEvent::PeerTrackAdded(peer, track, meta) => {
                self.cluster_track_map.insert((peer.clone(), track.clone()), meta.kind);
                self.push_rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                    peer_hash: hash_str(&peer) as u32,
                    peer,
                    kind: meta.kind,
                    track,
                    state: Some(meta),
                }));
            }
            ClusterEndpointIncomingEvent::PeerTrackUpdated(peer, track, meta) => {
                self.cluster_track_map.insert((peer.clone(), track.clone()), meta.kind);
                self.push_rpc(EndpointRpcOut::TrackUpdated(TrackInfo {
                    peer_hash: hash_str(&peer) as u32,
                    peer,
                    kind: meta.kind,
                    track,
                    state: Some(meta),
                }));
            }
            ClusterEndpointIncomingEvent::PeerTrackRemoved(peer, track) => {
                if let Some(kind) = self.cluster_track_map.remove(&(peer.clone(), track.clone())) {
                    self.push_rpc(EndpointRpcOut::TrackRemoved(TrackInfo {
                        peer_hash: hash_str(&peer) as u32,
                        peer,
                        kind,
                        track,
                        state: None,
                    }));
                }
            }
            ClusterEndpointIncomingEvent::LocalTrackEvent(track_id, event) => {
                if let Some(track) = self.local_tracks.get_mut(&track_id) {
                    track.on_cluster_event(now_ms, event);
                } else {
                    log::warn!("[EndpointInternal] local track not found {:?}", track_id);
                }
            }
            ClusterEndpointIncomingEvent::RemoteTrackEvent(track_id, event) => {
                if let Some(track) = self.remote_tracks.get_mut(&track_id) {
                    track.on_cluster_event(event);
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?} for handle cluster event", track_id);
                }
            }
        }

        self.pop_internal(now_ms);
    }

    pub fn pop_action(&mut self) -> Option<MediaInternalAction> {
        self.output_actions.pop_front()
    }

    fn process_rpc(&mut self, rpc: EndpointRpcIn) {
        match rpc {
            EndpointRpcIn::PeerClose => {
                self.push_internal(MediaEndpointInternalEvent::ConnectionCloseRequest);
            }
            EndpointRpcIn::SubscribePeer(peer) => {}
            EndpointRpcIn::UnsubscribePeer(peer) => {}
            EndpointRpcIn::MixMinusSourceAdd(_) => todo!(),
            EndpointRpcIn::MixMinusSourceRemove(_) => todo!(),
            EndpointRpcIn::MixMinusToggle(_) => todo!(),
        }
    }

    fn pop_tracks_actions(&mut self, now_ms: u64) -> bool {
        let mut has_event = false;
        let mut should_pop_bitrate_allocation = false;
        for (track_id, track) in self.local_tracks.iter_mut() {
            while let Some(action) = track.pop_action() {
                has_event = true;
                match action {
                    LocalTrackOutput::Internal(event) => match event {
                        LocalTrackInternalOutputEvent::SourceSet(priority) => {
                            self.bitrate_allocator.add_local_track(*track_id, priority);
                            should_pop_bitrate_allocation = true;
                        }
                        LocalTrackInternalOutputEvent::SourceStats(stats) => {
                            self.bitrate_allocator.update_source_bitrate(*track_id, stats);
                            should_pop_bitrate_allocation = true;
                        }
                        LocalTrackInternalOutputEvent::SourceRemove => {
                            self.bitrate_allocator.remove_local_track(*track_id);
                            should_pop_bitrate_allocation = true;
                        }
                        LocalTrackInternalOutputEvent::Limit(limit) => {
                            self.bitrate_allocator.update_local_track_limit(*track_id, limit);
                            should_pop_bitrate_allocation = true;
                        }
                    },
                    LocalTrackOutput::Transport(event) => {
                        self.output_actions.push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::LocalTrackEvent(*track_id, event)));
                    }
                    LocalTrackOutput::Cluster(event) => {
                        self.output_actions
                            .push_back(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(*track_id, event)));
                    }
                }
            }
        }

        for (track_id, track) in self.remote_tracks.iter_mut() {
            while let Some(action) = track.pop_action() {
                has_event = true;
                match action {
                    RemoteTrackOutput::Transport(event) => {
                        self.output_actions.push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::RemoteTrackEvent(*track_id, event)));
                    }
                    RemoteTrackOutput::Cluster(event) => {
                        let cluster_track_uuid = track.cluster_track_uuid();
                        self.output_actions
                            .push_back(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(*track_id, cluster_track_uuid, event)));
                    }
                }
            }
        }

        if should_pop_bitrate_allocation {
            self.pop_bitrate_allocation_action(now_ms);
        }

        has_event
    }

    fn pop_local_track_actions(&mut self, now_ms: u64, track_id: TrackId, track: &mut LocalTrack) -> bool {
        let mut has_event = false;
        while let Some(action) = track.pop_action() {
            has_event = true;
            match action {
                LocalTrackOutput::Internal(event) => match event {
                    LocalTrackInternalOutputEvent::SourceSet(priority) => {
                        self.bitrate_allocator.add_local_track(track_id, priority);
                        self.pop_bitrate_allocation_action(now_ms);
                    }
                    LocalTrackInternalOutputEvent::SourceStats(stats) => {
                        self.bitrate_allocator.update_source_bitrate(track_id, stats);
                        self.pop_bitrate_allocation_action(now_ms);
                    }
                    LocalTrackInternalOutputEvent::SourceRemove => {
                        self.bitrate_allocator.remove_local_track(track_id);
                        self.pop_bitrate_allocation_action(now_ms);
                    }
                    LocalTrackInternalOutputEvent::Limit(limit) => {
                        self.bitrate_allocator.update_local_track_limit(track_id, limit);
                        self.pop_bitrate_allocation_action(now_ms);
                    }
                },
                LocalTrackOutput::Transport(event) => {
                    self.output_actions.push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::LocalTrackEvent(track_id, event)));
                }
                LocalTrackOutput::Cluster(event) => {
                    self.output_actions
                        .push_back(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(track_id, event)));
                }
            }
        }

        has_event
    }

    fn pop_remote_track_actions(&mut self, track_id: TrackId, track: &mut RemoteTrack) -> bool {
        let mut has_event = false;

        while let Some(action) = track.pop_action() {
            has_event = true;
            match action {
                RemoteTrackOutput::Transport(event) => {
                    self.output_actions.push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::RemoteTrackEvent(track_id, event)));
                }
                RemoteTrackOutput::Cluster(event) => {
                    let cluster_track_uuid = track.cluster_track_uuid();
                    self.output_actions
                        .push_back(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(track_id, cluster_track_uuid, event)));
                }
            }
        }

        has_event
    }

    fn pop_bitrate_allocation_action(&mut self, _now_ms: u64) -> bool {
        let mut has_event = false;
        while let Some(action) = self.bitrate_allocator.pop_action() {
            has_event = true;
            match action {
                BitrateAllocationAction::LimitLocalTrack(track_id, limit) => {
                    if let Some(track) = self.local_tracks.get_mut(&track_id) {
                        track.set_target(limit);
                    }
                }
                BitrateAllocationAction::LimitLocalTrackBitrate(track_id, bitrate) => {
                    if let Some(track) = self.local_tracks.get_mut(&track_id) {
                        track.set_bitrate(bitrate);
                    }
                }
                BitrateAllocationAction::ConfigEgressBitrate { current, desired } => {
                    self.output_actions
                        .push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::ConfigEgressBitrate { current, desired }));
                }
            }
        }

        has_event
    }

    /// Close this and cleanup everything
    /// This should be called when the endpoint is closed
    /// - Close all tracks
    pub fn before_drop(&mut self, now_ms: u64) {
        let local_tracks = std::mem::take(&mut self.local_tracks);
        for (track_id, mut track) in local_tracks {
            log::info!("[MediaEndpointInternal {}/{}] close local track {}", self.room_id, self.peer_id, track_id);
            track.close();
            self.pop_local_track_actions(now_ms, track_id, &mut track);
        }

        let remote_tracks = std::mem::take(&mut self.remote_tracks);
        for (track_id, mut track) in remote_tracks {
            log::info!("[MediaEndpointInternal {}/{}] close remote track {}", self.room_id, self.peer_id, track_id);
            track.close();
            self.pop_remote_track_actions(track_id, &mut track);
        }
    }
}

impl Drop for MediaEndpointInternal {
    fn drop(&mut self) {
        log::info!("[MediaEndpointInternal {}/{}] drop", self.room_id, self.peer_id);
        assert!(self.local_tracks.is_empty());
        assert!(self.remote_tracks.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use cluster::{generate_cluster_track_uuid, ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackOutgoingEvent, ClusterRemoteTrackOutgoingEvent, ClusterTrackMeta};
    use transport::{
        LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, RemoteTrackIncomingEvent, RequestKeyframeKind, TrackMeta, TransportIncomingEvent, TransportOutgoingEvent, TransportStateEvent,
    };

    use crate::{
        endpoint_wrap::internal::{bitrate_limiter::BitrateLimiterType, MediaEndpointInternalEvent, MediaInternalAction, DEFAULT_BITRATE_OUT_BPS},
        rpc::{LocalTrackRpcIn, LocalTrackRpcOut, ReceiverSwitch, RemoteStream, TrackInfo},
        EndpointRpcOut, RpcRequest, RpcResponse,
    };

    use super::MediaEndpointInternal;

    #[test]
    fn should_fire_cluster_when_remote_track_added_then_close() {
        let mut endpoint = MediaEndpointInternal::new("room1", "peer1", BitrateLimiterType::DynamicWithConsumers);

        let cluster_track_uuid = generate_cluster_track_uuid("room1", "peer1", "audio_main");
        endpoint.on_transport(0, TransportIncomingEvent::RemoteTrackAdded("audio_main".to_string(), 100, TrackMeta::new_audio(None)));

        assert_eq!(endpoint.remote_tracks.len(), 1);

        // should handle pkt
        let pkt = MediaPacket::simple_audio(1, 1000, vec![1, 2, 3]);
        endpoint.on_transport(0, TransportIncomingEvent::RemoteTrackEvent(100, RemoteTrackIncomingEvent::MediaPacket(pkt.clone())));
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackAdded("audio_main".to_string(), ClusterTrackMeta::default_audio())
            )))
        );
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackMedia(pkt)
            )))
        );

        // close should fire cluster event
        endpoint.before_drop(0);

        // should output cluster event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackRemoved("audio_main".to_string())
            )))
        );
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_cluster_when_remote_track_added_then_removed() {
        let mut endpoint = MediaEndpointInternal::new("room1", "peer1", BitrateLimiterType::DynamicWithConsumers);

        let cluster_track_uuid = generate_cluster_track_uuid("room1", "peer1", "audio_main");
        endpoint.on_transport(0, TransportIncomingEvent::RemoteTrackAdded("audio_main".to_string(), 100, TrackMeta::new_audio(None)));

        assert_eq!(endpoint.remote_tracks.len(), 1);

        // should handle pkt
        let pkt = MediaPacket::simple_audio(1, 1000, vec![1, 2, 3]);
        endpoint.on_transport(0, TransportIncomingEvent::RemoteTrackEvent(100, RemoteTrackIncomingEvent::MediaPacket(pkt.clone())));
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackAdded("audio_main".to_string(), ClusterTrackMeta::default_audio())
            )))
        );
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackMedia(pkt)
            )))
        );

        endpoint.on_transport(0, TransportIncomingEvent::RemoteTrackRemoved("audio_main".to_string(), 100));

        // should output cluster event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackRemoved("audio_main".to_string())
            )))
        );
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_rpc_when_cluster_track_added() {
        let mut endpoint = MediaEndpointInternal::new("room1", "peer1", BitrateLimiterType::DynamicWithConsumers);

        endpoint.on_cluster(
            0,
            ClusterEndpointIncomingEvent::PeerTrackAdded("peer2".to_string(), "audio_main".to_string(), ClusterTrackMeta::default_audio()),
        );

        // should output rpc event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo::new_audio(
                "peer2",
                "audio_main",
                Some(ClusterTrackMeta::default_audio())
            )))))
        );
        assert_eq!(endpoint.pop_action(), None);

        endpoint.on_cluster(0, ClusterEndpointIncomingEvent::PeerTrackRemoved("peer2".to_string(), "audio_main".to_string()));

        // should output rpc event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackRemoved(TrackInfo::new_audio(
                "peer2", "audio_main", None
            )))))
        );
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_disconnect_when_transport_disconnect() {
        let mut endpoint = MediaEndpointInternal::new("room1", "peer1", BitrateLimiterType::DynamicWithConsumers);

        endpoint.on_transport(0, TransportIncomingEvent::State(TransportStateEvent::Disconnected));

        // should output internal event
        assert_eq!(endpoint.pop_action(), Some(MediaInternalAction::Internal(MediaEndpointInternalEvent::ConnectionClosed)));
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_answer_rpc() {
        let mut endpoint = MediaEndpointInternal::new("room1", "peer1", BitrateLimiterType::DynamicWithConsumers);

        endpoint.on_transport(0, TransportIncomingEvent::LocalTrackAdded("video_0".to_string(), 1, TrackMeta::new_video(None)));

        // should output rpc response and subscribe when rpc switch
        endpoint.on_transport(
            0,
            TransportIncomingEvent::LocalTrackEvent(
                1,
                LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
                    req_id: 1,
                    data: ReceiverSwitch {
                        id: "video_0".to_string(),
                        priority: 1000,
                        remote: RemoteStream {
                            peer: "peer2".to_string(),
                            stream: "video_main".to_string(),
                        },
                    },
                })),
            ),
        );

        endpoint.on_tick(0);

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::Subscribe("peer2".to_string(), "video_main".to_string())
            )))
        );

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                1,
                LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(1, true)))
            )))
        );

        // dont fire this event without remote tracks
        // assert_eq!(
        //     endpoint.pop_action(),
        //     Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::LimitIngressBitrate(IDLE_BITRATE_RECV_LIMIT)))
        // );

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::ConfigEgressBitrate {
                current: 0,
                desired: 80_000 * 6 / 5 //Default of single stream
            }))
        );

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::LimitBitrate(DEFAULT_BITRATE_OUT_BPS)
            )))
        );

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::RequestKeyFrame(RequestKeyframeKind::Pli)
            )))
        );

        assert_eq!(endpoint.pop_action(), None);

        endpoint.before_drop(0);

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::Unsubscribe("peer2".to_string(), "video_main".to_string())
            )))
        );
    }

    #[test]
    fn should_forward_remote_track_stats() {
        //TODO
    }
}
