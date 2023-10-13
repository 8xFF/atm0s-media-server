use std::collections::{HashMap, VecDeque};

use cluster::{generate_cluster_track_uuid, ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterTrackMeta};
use transport::{MediaIncomingEvent, MediaKind, MediaOutgoingEvent, TrackId};
use utils::hash_str;

use crate::rpc::{EndpointRpcIn, EndpointRpcOut, RpcResponse, TrackInfo};

use self::{
    local_track::{LocalTrack, LocalTrackSource},
    remote_track::RemoteTrack,
};

mod local_track;
mod remote_track;

pub enum MediaEndpointInteralEvent {}

pub enum MediaInternalAction {
    Internal(MediaEndpointInteralEvent),
    Endpoint(MediaOutgoingEvent<EndpointRpcOut>),
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
        self.output_actions.push_back(MediaInternalAction::Endpoint(MediaOutgoingEvent::Rpc(rpc)));
    }

    fn push_endpoint(&mut self, event: MediaOutgoingEvent<EndpointRpcOut>) {
        self.output_actions.push_back(MediaInternalAction::Endpoint(event));
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
            while let Some(event) = track.pop_action() {
                //TODO
            }
        }

        for (_, track) in self.remote_tracks.iter_mut() {
            track.on_tick(now_ms);
            while let Some(event) = track.pop_action() {
                //TODO
            }
        }
    }

    pub fn on_transport(&mut self, event: MediaIncomingEvent<EndpointRpcIn>) {
        match event {
            MediaIncomingEvent::Connected => {}
            MediaIncomingEvent::Reconnecting => {}
            MediaIncomingEvent::Reconnected => {}
            MediaIncomingEvent::Disconnected => {}
            MediaIncomingEvent::Continue => {}
            MediaIncomingEvent::Rpc(rpc) => self.process_rpc(rpc),
            MediaIncomingEvent::Stats(stats) => todo!(),
            MediaIncomingEvent::RemoteTrackAdded(track_name, track_id, meta) => {
                if !self.remote_tracks.contains_key(&track_id) {
                    let track = RemoteTrack::new(track_id, &track_name, meta);
                    self.push_cluster(ClusterEndpointOutgoingEvent::TrackAdded(track_name, track.cluster_meta()));
                    self.remote_tracks.insert(track_id, track);
                } else {
                    log::warn!("[EndpointInternal] remote track already exists {:?}", track_id);
                }
            }
            MediaIncomingEvent::RemoteTrackMedia(track_id, pkt) => {
                if let Some(track) = self.remote_tracks.get_mut(&track_id) {
                    if let Some((cluster_track_uuid, pkt)) = track.on_pkt(pkt) {
                        self.push_cluster(ClusterEndpointOutgoingEvent::TrackMedia(cluster_track_uuid, pkt));
                    }
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?}", track_id);
                }
            }
            MediaIncomingEvent::RemoteTrackRemoved(track_name, track_id, meta) => {
                if let Some(track) = self.remote_tracks.remove(&track_id) {
                    self.push_cluster(ClusterEndpointOutgoingEvent::TrackRemoved(track_name));
                } else {
                    log::warn!("[EndpointInternal] remote track not found {:?}", track_id);
                }
            }
            MediaIncomingEvent::LocalTrackAdded(track_name, track_id, meta) => {
                if !self.local_tracks.contains_key(&track_id) {
                    let track = LocalTrack::new(&self.room_id, &self.peer_id, track_id, &track_name, meta);
                    self.local_tracks.insert(track_id, track);
                    self.local_track_map.insert(track_name, track_id);
                } else {
                    log::warn!("[EndpointInternal] local track already exists {:?}", track_id);
                }
            }
            MediaIncomingEvent::LocalTrackRemoved(track_name, track_id) => {
                if let Some(track) = self.local_tracks.remove(&track_id) {
                    self.local_track_map.remove(&track_name);
                } else {
                    log::warn!("[EndpointInternal] local track not found {:?}", track_id);
                }
            }
            _ => {
                panic!("not implement {:?}", event)
            }
        }
    }

    pub fn on_cluster(&mut self, event: ClusterEndpointIncomingEvent) {
        match event {
            ClusterEndpointIncomingEvent::PeerTrackMedia(cluster_track_uuid, pkt) => {
                //TODO reduce for check here
                let mut out_events = vec![];
                for (_, track) in self.local_tracks.iter_mut() {
                    if let Some(track_source_track_uuid) = track.source_uuid() {
                        if track_source_track_uuid == cluster_track_uuid {
                            if let Some((track_id, pkt)) = track.on_pkt(&pkt) {
                                out_events.push(MediaOutgoingEvent::<EndpointRpcOut>::Media(track_id.clone(), pkt));
                            }
                        }
                    }
                }
                for event in out_events {
                    self.push_endpoint(event);
                }
            }
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
                    self.push_rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                        peer_hash: hash_str(&peer) as u32,
                        peer,
                        kind,
                        track,
                        state: None,
                    }));
                }
            }
        }
    }

    pub fn pop_action(&mut self) -> Option<MediaInternalAction> {
        self.output_actions.pop_front()
    }

    fn process_rpc(&mut self, rpc: EndpointRpcIn) {
        match rpc {
            EndpointRpcIn::PeerClose => {
                todo!()
            }
            EndpointRpcIn::SenderToggle(req) => {
                todo!()
            }
            EndpointRpcIn::ReceiverSwitch(req) => {
                if let Some(track_id) = self.local_track_map.get(&req.data.id) {
                    if let Some(track) = self.local_tracks.get_mut(track_id) {
                        //TODO handle priority
                        let consumer_id = track.consumer_uuid();
                        let cluster_track_uuid = generate_cluster_track_uuid(&self.room_id, &req.data.remote.peer, &req.data.remote.stream);
                        let old_source = track.repace_source(Some(LocalTrackSource::new(&req.data.remote.peer, &req.data.remote.stream, cluster_track_uuid)));
                        if let Some(old_source) = old_source {
                            self.push_cluster(ClusterEndpointOutgoingEvent::UnsubscribeTrack(old_source.peer, old_source.track, old_source.uuid));
                        }
                        self.push_cluster(ClusterEndpointOutgoingEvent::SubscribeTrack(req.data.remote.peer, req.data.remote.stream, consumer_id));
                        self.push_rpc(EndpointRpcOut::ReceiverSwitchRes(RpcResponse::success(req.req_id, true)));
                    } else {
                        log::warn!("[EndpointInternal] local track not found {:?}", track_id);
                        self.push_rpc(EndpointRpcOut::ReceiverSwitchRes(RpcResponse::error(req.req_id)));
                    }
                } else {
                    log::warn!("[EndpointInternal] local track not found {:?}", req.data.id);
                    self.push_rpc(EndpointRpcOut::ReceiverSwitchRes(RpcResponse::error(req.req_id)));
                }
            }
            EndpointRpcIn::ReceiverLimit(req) => {
                if let Some(track_id) = self.local_track_map.get(&req.data.id) {
                    if let Some(track) = self.local_tracks.get_mut(track_id) {
                        track.limit(req.data.limit);
                        self.push_rpc(EndpointRpcOut::ReceiverLimitRes(RpcResponse::success(req.req_id, true)));
                    } else {
                        self.push_rpc(EndpointRpcOut::ReceiverLimitRes(RpcResponse::error(req.req_id)));
                    }
                } else {
                    self.push_rpc(EndpointRpcOut::ReceiverLimitRes(RpcResponse::error(req.req_id)));
                }
            }
            EndpointRpcIn::ReceiverDisconnect(_) => todo!(),
            EndpointRpcIn::MixMinusSourceAdd(_) => todo!(),
            EndpointRpcIn::MixMinusSourceRemove(_) => todo!(),
            EndpointRpcIn::MixMinusToggle(_) => todo!(),
        }
    }
}
