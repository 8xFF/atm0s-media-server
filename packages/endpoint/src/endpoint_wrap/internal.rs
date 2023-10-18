use std::collections::{HashMap, VecDeque};

use cluster::{ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent};
use transport::{MediaKind, TrackId, TransportIncomingEvent, TransportOutgoingEvent, TransportStateEvent};
use utils::hash_str;

use crate::rpc::{EndpointRpcIn, EndpointRpcOut, LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut, TrackInfo};

use self::{
    local_track::{LocalTrack, LocalTrackOutput},
    remote_track::{RemoteTrack, RemoteTrackOutput},
};

mod local_track;
mod remote_track;

#[derive(Debug, PartialEq, Eq)]
pub enum MediaEndpointInteralEvent {
    ConnectionClosed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MediaInternalAction {
    Internal(MediaEndpointInteralEvent),
    Endpoint(TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>),
    Cluster(ClusterEndpointOutgoingEvent),
}

pub struct MediaEndpointInteral {
    room_id: String,
    peer_id: String,
    cluster_track_map: HashMap<(String, String), MediaKind>,
    local_track_map: HashMap<String, TrackId>,
    output_actions: VecDeque<MediaInternalAction>,
    local_tracks: HashMap<TrackId, LocalTrack>,
    remote_tracks: HashMap<TrackId, RemoteTrack>,
}

impl MediaEndpointInteral {
    pub fn new(room_id: &str, peer_id: &str) -> Self {
        log::info!("[MediaEndpointInteral {}/{}] create", room_id, peer_id);
        Self {
            room_id: room_id.into(),
            peer_id: peer_id.into(),
            cluster_track_map: HashMap::new(),
            local_track_map: HashMap::new(),
            output_actions: VecDeque::with_capacity(100),
            local_tracks: HashMap::new(),
            remote_tracks: HashMap::new(),
        }
    }

    fn push_rpc(&mut self, rpc: EndpointRpcOut) {
        self.output_actions.push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::Rpc(rpc)));
    }

    fn push_cluster(&mut self, event: ClusterEndpointOutgoingEvent) {
        self.output_actions.push_back(MediaInternalAction::Cluster(event));
    }

    fn push_internal(&mut self, event: MediaEndpointInteralEvent) {
        self.output_actions.push_back(MediaInternalAction::Internal(event));
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        for (_, track) in self.local_tracks.iter_mut() {
            track.on_tick(now_ms);
        }

        for (_, track) in self.remote_tracks.iter_mut() {
            track.on_tick(now_ms);
        }

        self.pop_tracks_actions();
    }

    pub fn on_transport(&mut self, event: TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>) {
        match event {
            TransportIncomingEvent::State(state) => {
                log::info!("[EndpointInternal] switch state to {:?}", state);
                match state {
                    TransportStateEvent::Connected => {}
                    TransportStateEvent::Reconnecting => {}
                    TransportStateEvent::Reconnected => {}
                    TransportStateEvent::Disconnected => {
                        self.push_internal(MediaEndpointInteralEvent::ConnectionClosed);
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
                    self.push_cluster(ClusterEndpointOutgoingEvent::TrackAdded(track_id, track_name, track.cluster_meta()));
                    self.remote_tracks.insert(track_id, track);
                } else {
                    log::warn!("[EndpointInternal] remote track already exists {:?}", track_id);
                }
            }
            TransportIncomingEvent::RemoteTrackEvent(track_id, event) => {
                if let Some(track) = self.remote_tracks.get_mut(&track_id) {
                    track.on_transport_event(event);
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?}", track_id);
                }
            }
            TransportIncomingEvent::RemoteTrackRemoved(track_name, track_id) => {
                if let Some(mut track) = self.remote_tracks.remove(&track_id) {
                    track.close();
                    self.pop_remote_track_actions(track_id, &mut track);
                    self.push_cluster(ClusterEndpointOutgoingEvent::TrackRemoved(track_id, track_name));
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?}", track_id);
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
                    log::warn!("[EndpointInternal] remote track not found {:?}", track_id);
                }
            }
            TransportIncomingEvent::LocalTrackRemoved(track_name, track_id) => {
                if let Some(mut track) = self.local_tracks.remove(&track_id) {
                    track.close();
                    self.pop_local_track_actions(track_id, &mut track);
                    self.local_track_map.remove(&track_name);
                } else {
                    log::warn!("[EndpointInternal] local track not found {:?}", track_id);
                }
            }
        }

        self.pop_tracks_actions();
    }

    pub fn on_cluster(&mut self, event: ClusterEndpointIncomingEvent) {
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
                    track.on_cluster_event(event);
                } else {
                    log::warn!("[EndpointInternal] local track not found {:?}", track_id);
                }
            }
            ClusterEndpointIncomingEvent::RemoteTrackEvent(track_id, event) => {
                if let Some(track) = self.remote_tracks.get_mut(&track_id) {
                    track.on_cluster_event(event);
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?}", track_id);
                }
            }
        }

        self.pop_tracks_actions();
    }

    pub fn pop_action(&mut self) -> Option<MediaInternalAction> {
        self.output_actions.pop_front()
    }

    fn process_rpc(&mut self, rpc: EndpointRpcIn) {
        match rpc {
            EndpointRpcIn::PeerClose => {
                todo!()
            }
            EndpointRpcIn::MixMinusSourceAdd(_) => todo!(),
            EndpointRpcIn::MixMinusSourceRemove(_) => todo!(),
            EndpointRpcIn::MixMinusToggle(_) => todo!(),
        }
    }

    fn pop_tracks_actions(&mut self) {
        for (track_id, track) in self.local_tracks.iter_mut() {
            while let Some(action) = track.pop_action() {
                match action {
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
    }

    fn pop_local_track_actions(&mut self, track_id: TrackId, track: &mut LocalTrack) {
        while let Some(action) = track.pop_action() {
            match action {
                LocalTrackOutput::Transport(event) => {
                    self.output_actions.push_back(MediaInternalAction::Endpoint(TransportOutgoingEvent::LocalTrackEvent(track_id, event)));
                }
                LocalTrackOutput::Cluster(event) => {
                    self.output_actions
                        .push_back(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(track_id, event)));
                }
            }
        }
    }

    fn pop_remote_track_actions(&mut self, track_id: TrackId, track: &mut RemoteTrack) {
        while let Some(action) = track.pop_action() {
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
    }

    /// Close this and cleanup everything
    /// This should be called when the endpoint is closed
    /// - Close all tracks
    pub fn close(&mut self) {
        let local_tracks = std::mem::take(&mut self.local_tracks);
        for (track_id, mut track) in local_tracks {
            log::info!("[MediaEndpointInteral {}/{}] close local track {}", self.room_id, self.peer_id, track_id);
            track.close();
            self.pop_local_track_actions(track_id, &mut track);
        }

        let remote_tracks = std::mem::take(&mut self.remote_tracks);
        for (track_id, mut track) in remote_tracks {
            log::info!("[MediaEndpointInteral {}/{}] close remote track {}", self.room_id, self.peer_id, track_id);
            track.close();
            self.pop_remote_track_actions(track_id, &mut track);
            self.push_cluster(ClusterEndpointOutgoingEvent::TrackRemoved(track_id, track.track_name().to_string()));
        }
    }
}

impl Drop for MediaEndpointInteral {
    fn drop(&mut self) {
        log::info!("[MediaEndpointInteral {}/{}] drop", self.room_id, self.peer_id);
        assert!(self.local_tracks.is_empty());
        assert!(self.remote_tracks.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use cluster::{generate_cluster_track_uuid, ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackOutgoingEvent, ClusterRemoteTrackOutgoingEvent, ClusterTrackMeta};
    use transport::{
        LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaKind, MediaSampleRate, RemoteTrackIncomingEvent, TrackMeta, TransportIncomingEvent, TransportOutgoingEvent, TransportStateEvent,
    };
    use utils::hash_str;

    use crate::{
        endpoint_wrap::internal::{MediaEndpointInteralEvent, MediaInternalAction},
        rpc::{LocalTrackRpcIn, LocalTrackRpcOut, ReceiverSwitch, RemoteStream, TrackInfo},
        EndpointRpcOut, RpcRequest, RpcResponse,
    };

    use super::MediaEndpointInteral;

    #[test]
    fn should_fire_cluster_when_remote_track_added_then_close() {
        let mut endpoint = MediaEndpointInteral::new("room1", "peer1");

        let cluster_track_uuid = generate_cluster_track_uuid("room1", "peer1", "audio_main");
        endpoint.on_transport(TransportIncomingEvent::RemoteTrackAdded(
            "audio_main".to_string(),
            100,
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        ));

        assert_eq!(endpoint.remote_tracks.len(), 1);

        // should output cluster event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::TrackAdded(
                100,
                "audio_main".to_string(),
                ClusterTrackMeta {
                    kind: MediaKind::Audio,
                    scaling: "Single".to_string(),
                    layers: vec![],
                    status: cluster::ClusterTrackStatus::Connected,
                    active: true,
                    label: None,
                }
            )))
        );
        assert_eq!(endpoint.pop_action(), None);

        // should handle pkt
        let pkt = transport::MediaPacket {
            pt: 111,
            seq_no: 1,
            time: 1000,
            marker: true,
            ext_vals: transport::MediaPacketExtensions {
                abs_send_time: None,
                transport_cc: None,
            },
            nackable: true,
            payload: vec![1, 2, 3],
        };
        endpoint.on_transport(TransportIncomingEvent::RemoteTrackEvent(100, RemoteTrackIncomingEvent::MediaPacket(pkt.clone())));
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt)
            )))
        );

        // close should fire cluster event
        endpoint.close();

        // should output cluster event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::TrackRemoved(100, "audio_main".to_string())))
        );
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_cluster_when_remote_track_added_then_removed() {
        let mut endpoint = MediaEndpointInteral::new("room1", "peer1");

        let cluster_track_uuid = generate_cluster_track_uuid("room1", "peer1", "audio_main");
        endpoint.on_transport(TransportIncomingEvent::RemoteTrackAdded(
            "audio_main".to_string(),
            100,
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        ));

        assert_eq!(endpoint.remote_tracks.len(), 1);

        // should output cluster event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::TrackAdded(
                100,
                "audio_main".to_string(),
                ClusterTrackMeta {
                    kind: MediaKind::Audio,
                    scaling: "Single".to_string(),
                    layers: vec![],
                    status: cluster::ClusterTrackStatus::Connected,
                    active: true,
                    label: None,
                }
            )))
        );
        assert_eq!(endpoint.pop_action(), None);

        // should handle pkt
        let pkt = transport::MediaPacket {
            pt: 111,
            seq_no: 1,
            time: 1000,
            marker: true,
            ext_vals: transport::MediaPacketExtensions {
                abs_send_time: None,
                transport_cc: None,
            },
            nackable: true,
            payload: vec![1, 2, 3],
        };
        endpoint.on_transport(TransportIncomingEvent::RemoteTrackEvent(100, RemoteTrackIncomingEvent::MediaPacket(pkt.clone())));
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                100,
                cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::MediaPacket(pkt)
            )))
        );

        endpoint.on_transport(TransportIncomingEvent::RemoteTrackRemoved("audio_main".to_string(), 100));

        // should output cluster event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::TrackRemoved(100, "audio_main".to_string())))
        );
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_rpc_when_cluster_track_added() {
        let mut endpoint = MediaEndpointInteral::new("room1", "peer1");

        endpoint.on_cluster(ClusterEndpointIncomingEvent::PeerTrackAdded(
            "peer2".to_string(),
            "audio_main".to_string(),
            ClusterTrackMeta {
                kind: MediaKind::Audio,
                scaling: "Single".to_string(),
                layers: vec![],
                status: cluster::ClusterTrackStatus::Connected,
                active: true,
                label: None,
            },
        ));

        // should output rpc event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                peer_hash: hash_str("peer2") as u32,
                peer: "peer2".to_string(),
                kind: MediaKind::Audio,
                track: "audio_main".to_string(),
                state: Some(ClusterTrackMeta {
                    kind: MediaKind::Audio,
                    scaling: "Single".to_string(),
                    layers: vec![],
                    status: cluster::ClusterTrackStatus::Connected,
                    active: true,
                    label: None,
                }),
            }))))
        );
        assert_eq!(endpoint.pop_action(), None);

        endpoint.on_cluster(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer2".to_string(), "audio_main".to_string()));

        // should output rpc event
        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackRemoved(TrackInfo {
                peer_hash: hash_str("peer2") as u32,
                peer: "peer2".to_string(),
                kind: MediaKind::Audio,
                track: "audio_main".to_string(),
                state: None,
            }))))
        );
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_disconnect_when_transport_disconnect() {
        let mut endpoint = MediaEndpointInteral::new("room1", "peer1");

        endpoint.on_transport(TransportIncomingEvent::State(TransportStateEvent::Disconnected));

        // should output internal event
        assert_eq!(endpoint.pop_action(), Some(MediaInternalAction::Internal(MediaEndpointInteralEvent::ConnectionClosed)));
        assert_eq!(endpoint.pop_action(), None);
    }

    #[test]
    fn should_fire_answer_rpc() {
        let mut endpoint = MediaEndpointInteral::new("room1", "peer1");

        endpoint.on_transport(TransportIncomingEvent::LocalTrackAdded(
            "audio_0".to_string(),
            1,
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        ));

        // should output rpc response and subscribe when rpc switch
        endpoint.on_transport(TransportIncomingEvent::LocalTrackEvent(
            1,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
                req_id: 1,
                data: ReceiverSwitch {
                    id: "audio_0".to_string(),
                    priority: 1000,
                    remote: RemoteStream {
                        peer: "peer2".to_string(),
                        stream: "audio_main".to_string(),
                    },
                },
            })),
        ));

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::Subscribe("peer2".to_string(), "audio_main".to_string())
            )))
        );

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Endpoint(TransportOutgoingEvent::LocalTrackEvent(
                1,
                LocalTrackOutgoingEvent::Rpc(LocalTrackRpcOut::SwitchRes(RpcResponse::success(1, true)))
            )))
        );

        endpoint.close();

        assert_eq!(
            endpoint.pop_action(),
            Some(MediaInternalAction::Cluster(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::Unsubscribe("peer2".to_string(), "audio_main".to_string())
            )))
        );
    }
}
