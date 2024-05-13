//!
//! Medata part takecare of how cluster will store peer, track info.
//! We have 3 level: Full, Track only and Manual
//!
//! - Full: subscribe on both peer and track information
//! - Track only: subscribe on track info, this method is useful with large users application like broadcast or webinar
//! - Manual: client manual call subscribe on which peer it interested in, this method is useful with some spartial audio application
//!

use std::{collections::VecDeque, fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::features::dht_kv::{self, Map, MapControl, MapEvent};
use media_server_protocol::endpoint::{PeerId, PeerInfo, PeerMeta, RoomInfoPublish, RoomInfoSubscribe, TrackInfo, TrackMeta, TrackName};
use sans_io_runtime::{return_if_none, TaskSwitcherChild};
use smallmap::{Map as SmallMap, Set as SmallSet};

use crate::{
    cluster::{id_generator, ClusterEndpointEvent, ClusterRoomHash},
    transport::RemoteTrackId,
};

struct PeerContainer {
    peer: PeerId,
    publish: RoomInfoPublish,
    sub_peers: SmallSet<PeerId>,
    pub_tracks: SmallMap<RemoteTrackId, TrackName>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output<Owner> {
    Kv(dht_kv::Control),
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    LastPeerLeaved,
}

pub struct RoomMetadata<Owner> {
    room: ClusterRoomHash,
    peers_map: Map,
    tracks_map: Map,
    peers: SmallMap<Owner, PeerContainer>,
    peers_map_subscribers: SmallSet<Owner>,
    tracks_map_subscribers: SmallSet<Owner>,
    //This is for storing list of owners subscribe manual a target track
    peers_tracks_subs: SmallMap<dht_kv::Map, SmallSet<Owner>>,
    cluster_peers: SmallMap<dht_kv::Key, PeerInfo>,
    cluster_tracks: SmallMap<dht_kv::Key, TrackInfo>,
    queue: VecDeque<Output<Owner>>,
}

impl<Owner: Hash + Eq + Copy + Debug> RoomMetadata<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            peers_map: id_generator::peers_map(room),
            tracks_map: id_generator::tracks_map(room),
            peers: SmallMap::new(),
            peers_map_subscribers: SmallMap::new(),
            tracks_map_subscribers: SmallMap::new(),
            peers_tracks_subs: SmallMap::new(),
            cluster_peers: SmallMap::new(),
            cluster_tracks: SmallMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn get_peer_from_owner(&self, owner: Owner) -> Option<PeerId> {
        Some(self.peers.get(&owner)?.peer.clone())
    }

    /// We put peer to list and register owner to peers and tracks list subscriber based on level
    pub fn on_join(&mut self, owner: Owner, peer: PeerId, meta: PeerMeta, publish: RoomInfoPublish, subscribe: RoomInfoSubscribe) {
        log::info!("[ClusterRoom {}] join peer ({peer})", self.room);
        // First let insert to peers cache for reuse when we need information of owner
        self.peers.insert(
            owner,
            PeerContainer {
                peer: peer.clone(),
                publish: publish.clone(),
                sub_peers: SmallSet::new(),
                pub_tracks: SmallMap::new(),
            },
        );
        let peer_key = id_generator::peers_key(&peer);

        // Let Set to peers_map if need need publisj.peer
        if publish.peer {
            self.queue
                .push_back(Output::Kv(dht_kv::Control::MapCmd(self.peers_map, MapControl::Set(peer_key, PeerInfo { peer, meta }.serialize()))))
        }
        // Let Sub to peers_map if need need subscribe.peers
        if subscribe.peers {
            self.peers_map_subscribers.insert(owner, ());
            log::info!("[ClusterRoom {}] next peer sub peers => restore {} remote peers", self.room, self.cluster_peers.len());

            // Restore already added peers
            for (_track_key, info) in self.cluster_peers.iter() {
                //TODO avoiding duplicate same peer
                self.queue
                    .push_back(Output::Endpoint(vec![owner], ClusterEndpointEvent::PeerJoined(info.peer.clone(), info.meta.clone())));
            }

            // If this is first peer which subscribed to peers_map, the should send Sub
            if self.peers_map_subscribers.len() == 1 {
                log::info!("[ClusterRoom {}] first peer sub peers map => subscribe", self.room);
                self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.peers_map, MapControl::Sub)));
            }
        }
        // Let Sub to tracks_map if need need subscribe.tracks
        if subscribe.tracks {
            self.tracks_map_subscribers.insert(owner, ());
            log::info!("[ClusterRoom {}] next peer sub tracks => restore {} remote tracks", self.room, self.cluster_tracks.len());

            // Restore already added tracks
            for (_track_key, info) in self.cluster_tracks.iter() {
                //TODO avoiding duplicate same peer
                self.queue.push_back(Output::Endpoint(
                    vec![owner],
                    ClusterEndpointEvent::TrackStarted(info.peer.clone(), info.track.clone(), info.meta.clone()),
                ));
            }

            // If this is first peer which subscribed to tracks_map, the should send Sub
            if self.tracks_map_subscribers.len() == 1 {
                log::info!("[ClusterRoom {}] first peer sub tracks map => subscribe", self.room);
                self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.tracks_map, MapControl::Sub)));
            }
        };
    }

    pub fn on_leave(&mut self, owner: Owner) {
        let peer = return_if_none!(self.peers.remove(&owner));
        log::info!("[ClusterRoom {}] leave peer {}", self.room, peer.peer);
        let peer_key = id_generator::peers_key(&peer.peer);
        // If remain remote tracks, must to delete from list.
        if peer.publish.peer {
            self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.peers_map, MapControl::Del(peer_key))))
        }

        // If remain remote tracks, must to delete from list.
        let peer_map = id_generator::peer_map(self.room, &peer.peer);
        for (_, track) in peer.pub_tracks.into_iter() {
            let track_key = id_generator::tracks_key(&peer.peer, &track);
            self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.tracks_map, MapControl::Del(track_key))));
            self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(peer_map, MapControl::Del(track_key))));
        }

        if self.peers_map_subscribers.remove(&owner).is_some() {
            if self.peers_map_subscribers.is_empty() {
                log::info!("[ClusterRoom {}] last peer unsub peers map => unsubscribe", self.room);
                self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.peers_map, MapControl::Unsub)));
            }
        }

        if self.tracks_map_subscribers.remove(&owner).is_some() {
            if self.tracks_map_subscribers.is_empty() {
                log::info!("[ClusterRoom {}] last peer unsub tracks map => unsubscribe", self.room);
                self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.tracks_map, MapControl::Unsub)));
            }
        }

        // check if this peer manual subscribe to some private peer map => need send Unsub
        for (target, _) in peer.sub_peers.into_iter() {
            let target_peer_map = id_generator::peer_map(self.room, &target);
            let subs = self.peers_tracks_subs.get_mut(&target_peer_map).expect("Should have private peer_map");
            subs.remove(&owner);
            if subs.is_empty() {
                self.peers_tracks_subs.remove(&target_peer_map);
                self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(target_peer_map, MapControl::Unsub)));
            }
        }

        if self.peers.is_empty() {
            log::info!("[ClusterRoom {}] last peer leaed => destroy metadata", self.room);
            self.queue.push_back(Output::LastPeerLeaved);
        }
    }

    pub fn on_subscribe_peer(&mut self, owner: Owner, target: PeerId) {
        let peer = self.peers.get_mut(&owner).expect("Should have peer");
        let target_peer_map = id_generator::peer_map(self.room, &target);
        let subs = self.peers_tracks_subs.entry(target_peer_map).or_default();
        let need_sub = subs.is_empty();
        subs.insert(owner, ());
        peer.sub_peers.insert(target, ());

        if need_sub {
            self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(target_peer_map, MapControl::Sub)));
        }
    }

    pub fn on_unsubscribe_peer(&mut self, owner: Owner, target: PeerId) {
        let peer = self.peers.get_mut(&owner).expect("Should have peer");
        let target_peer_map = id_generator::peer_map(self.room, &target);
        let subs = self.peers_tracks_subs.entry(target_peer_map).or_default();
        subs.remove(&owner);
        peer.sub_peers.remove(&target);
        if subs.is_empty() {
            self.peers_tracks_subs.remove(&target_peer_map);
            self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(target_peer_map, MapControl::Unsub)));
        }
    }

    pub fn on_track_publish(&mut self, owner: Owner, track_id: RemoteTrackId, track: TrackName, meta: TrackMeta) {
        let peer = return_if_none!(self.peers.get_mut(&owner));
        if peer.publish.tracks {
            let info = TrackInfo {
                peer: peer.peer.clone(),
                track: track.clone(),
                meta,
            };
            let track_key = id_generator::tracks_key(&peer.peer, &track);
            peer.pub_tracks.insert(track_id, track);

            let peer_map = id_generator::peer_map(self.room, &peer.peer);
            self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.tracks_map, MapControl::Set(track_key, info.serialize()))));
            self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(peer_map, MapControl::Set(track_key, info.serialize()))));
        }
    }

    pub fn on_track_unpublish(&mut self, owner: Owner, track_id: RemoteTrackId) {
        let peer = return_if_none!(self.peers.get_mut(&owner));
        let track = return_if_none!(peer.pub_tracks.remove(&track_id));
        let track_key = id_generator::tracks_key(&peer.peer, &track);

        let peer_map = id_generator::peer_map(self.room, &peer.peer);

        self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(self.tracks_map, MapControl::Del(track_key))));
        self.queue.push_back(Output::Kv(dht_kv::Control::MapCmd(peer_map, MapControl::Del(track_key))));
    }

    pub fn on_kv_event(&mut self, map: Map, event: MapEvent) {
        if self.peers_map == map {
            match event {
                dht_kv::MapEvent::OnSet(peer_key, _source, data) => self.on_peers_kv_event(peer_key, Some(data)),
                dht_kv::MapEvent::OnDel(peer_key, _source) => self.on_peers_kv_event(peer_key, None),
                dht_kv::MapEvent::OnRelaySelected(_) => {}
            }
        } else if self.tracks_map == map {
            match event {
                dht_kv::MapEvent::OnSet(track_key, _source, data) => self.on_tracks_kv_event(track_key, Some(data)),
                dht_kv::MapEvent::OnDel(track_key, _source) => self.on_tracks_kv_event(track_key, None),
                dht_kv::MapEvent::OnRelaySelected(_) => {}
            }
        } else if self.peers_tracks_subs.contains_key(&map) {
            match event {
                dht_kv::MapEvent::OnSet(track_key, _source, data) => self.on_peers_tracks_kv_event(map, track_key, Some(data)),
                dht_kv::MapEvent::OnDel(track_key, _source) => self.on_peers_tracks_kv_event(map, track_key, None),
                dht_kv::MapEvent::OnRelaySelected(_) => {}
            }
        }
    }

    fn on_peers_kv_event(&mut self, peer_key: dht_kv::Key, data: Option<Vec<u8>>) {
        let info = if let Some(data) = data {
            Some(return_if_none!(PeerInfo::deserialize(&data)))
        } else {
            None
        };

        let subscribers = self.peers_map_subscribers.iter().map(|a| a.0).collect::<Vec<_>>();
        if let Some(info) = info {
            log::info!("[ClusterRoom {}] cluster: peer {} joined => fire event to {:?}", self.room, info.peer, subscribers);
            self.cluster_peers.insert(peer_key, info.clone());
            if !subscribers.is_empty() {
                self.queue.push_back(Output::Endpoint(subscribers, ClusterEndpointEvent::PeerJoined(info.peer, info.meta)));
            }
        } else {
            let info = return_if_none!(self.cluster_peers.remove(&peer_key));
            log::info!("[ClusterRoom {}] cluster: peer ({}) leaved => fire event to {:?}", self.room, info.peer, subscribers);
            if !subscribers.is_empty() {
                self.queue.push_back(Output::Endpoint(subscribers, ClusterEndpointEvent::PeerLeaved(info.peer, info.meta)));
            }
        }
    }

    fn on_tracks_kv_event(&mut self, track: dht_kv::Key, data: Option<Vec<u8>>) {
        let info = if let Some(data) = data {
            Some(return_if_none!(TrackInfo::deserialize(&data)))
        } else {
            None
        };

        let subscribers = self.tracks_map_subscribers.iter().map(|a| a.0).collect::<Vec<_>>();
        if let Some(info) = info {
            log::info!(
                "[ClusterRoom {}] cluster: peer ({}) started track {}) => fire event to {:?}",
                self.room,
                info.peer,
                info.track,
                subscribers
            );
            self.cluster_tracks.insert(track, info.clone());
            if !subscribers.is_empty() {
                self.queue
                    .push_back(Output::Endpoint(subscribers, ClusterEndpointEvent::TrackStarted(info.peer, info.track, info.meta)));
            }
        } else {
            let info = return_if_none!(self.cluster_tracks.remove(&track));
            log::info!(
                "[ClusterRoom {}] cluster: peer ({}) stopped track {}) => fire event to {:?}",
                self.room,
                info.peer,
                info.track,
                subscribers
            );
            if !subscribers.is_empty() {
                self.queue
                    .push_back(Output::Endpoint(subscribers, ClusterEndpointEvent::TrackStopped(info.peer, info.track, info.meta)));
            }
        }
    }

    fn on_peers_tracks_kv_event(&mut self, peer_map: Map, track: dht_kv::Key, data: Option<Vec<u8>>) {
        let info = if let Some(data) = data {
            Some(return_if_none!(TrackInfo::deserialize(&data)))
        } else {
            None
        };

        let subscribers = return_if_none!(self.peers_tracks_subs.get(&peer_map)).iter().map(|a| a.0).collect::<Vec<_>>();
        if let Some(info) = info {
            log::info!(
                "[ClusterRoom {}] cluster: peer ({}) started track {}) => fire event to {:?}",
                self.room,
                info.peer,
                info.track,
                subscribers
            );
            self.cluster_tracks.insert(track, info.clone());
            self.queue
                .push_back(Output::Endpoint(subscribers, ClusterEndpointEvent::TrackStarted(info.peer, info.track, info.meta)));
        } else {
            let info = return_if_none!(self.cluster_tracks.remove(&track));
            log::info!(
                "[ClusterRoom {}] cluster: peer ({}) stopped track {}) => fire event to {:?}",
                self.room,
                info.peer,
                info.track,
                subscribers
            );
            self.queue
                .push_back(Output::Endpoint(subscribers, ClusterEndpointEvent::TrackStopped(info.peer, info.track, info.meta)));
        }
    }
}

impl<Owner> TaskSwitcherChild<Output<Owner>> for RoomMetadata<Owner> {
    type Time = Instant;
    fn pop_output(&mut self, _now: Instant) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use atm0s_sdn::features::dht_kv::{Control, MapControl, MapEvent};
    use media_server_protocol::endpoint::{PeerId, PeerInfo, PeerMeta, RoomInfoPublish, RoomInfoSubscribe, TrackInfo, TrackName};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::{
        cluster::{id_generator, ClusterEndpointEvent, ClusterRoomHash},
        transport::RemoteTrackId,
    };

    use super::{Output, RoomMetadata};

    /// Test correct get peer info
    #[test]
    fn correct_get_peer() {
        let room: ClusterRoomHash = 1.into();
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        let owner = 1;
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: false },
            RoomInfoSubscribe { peers: false, tracks: false },
        );

        assert_eq!(room_meta.get_peer_from_owner(1), Some(peer_id));
        assert_eq!(room_meta.get_peer_from_owner(2), None);
    }

    /// Test join as peer only => should subscribe peers, fire only peer
    /// After leave should unsubscribe only peers, and del
    #[test]
    fn join_peer_only() {
        let room: ClusterRoomHash = 1.into();
        let peers_map = id_generator::peers_map(room);
        let tracks_map = id_generator::tracks_map(room);
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        let peer_info = PeerInfo::new(peer_id.clone(), peer_meta.clone());
        let peer_key = id_generator::peers_key(&peer_id);
        let owner = 1;
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: true, tracks: false },
            RoomInfoSubscribe { peers: true, tracks: false },
        );
        assert_eq!(
            room_meta.pop_output(Instant::now()),
            Some(Output::Kv(Control::MapCmd(peers_map, MapControl::Set(peer_key, peer_info.serialize()))))
        );
        assert_eq!(room_meta.pop_output(Instant::now()), Some(Output::Kv(Control::MapCmd(peers_map, MapControl::Sub))));
        assert_eq!(room_meta.pop_output(Instant::now()), None);

        // should handle incoming event with only peer and reject track
        room_meta.on_kv_event(peers_map, MapEvent::OnSet(peer_key, 0, peer_info.serialize()));
        assert_eq!(
            room_meta.pop_output(Instant::now()),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::PeerJoined(peer_id.clone(), peer_meta.clone())))
        );
        assert_eq!(room_meta.pop_output(Instant::now()), None);

        let track_name: TrackName = "audio_main".to_string().into();
        let track_info = TrackInfo::simple_audio(peer_id.clone());
        let track_key = id_generator::tracks_key(&peer_id, &track_name);
        room_meta.on_kv_event(tracks_map, MapEvent::OnSet(track_key, 0, track_info.serialize()));
        assert_eq!(room_meta.pop_output(Instant::now()), None);

        // should only handle remove peer event, reject track
        room_meta.on_kv_event(tracks_map, MapEvent::OnDel(track_key, 0));
        assert_eq!(room_meta.pop_output(Instant::now()), None);

        room_meta.on_kv_event(peers_map, MapEvent::OnDel(peer_key, 0));
        assert_eq!(
            room_meta.pop_output(Instant::now()),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::PeerLeaved(peer_id.clone(), peer_info.meta)))
        );
        assert_eq!(room_meta.pop_output(Instant::now()), None);

        // peer leave should send unsub and del
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(Instant::now()), Some(Output::Kv(Control::MapCmd(peers_map, MapControl::Del(peer_key)))));
        assert_eq!(room_meta.pop_output(Instant::now()), Some(Output::Kv(Control::MapCmd(peers_map, MapControl::Unsub))));
        assert_eq!(room_meta.pop_output(Instant::now()), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(Instant::now()), None);
    }

    #[test]
    fn join_sub_peer_only_should_restore_old_peers() {
        let room: ClusterRoomHash = 1.into();
        let peers_map = id_generator::peers_map(room);
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);

        let peer2: PeerId = "peer2".to_string().into();
        let peer2_key = id_generator::peers_key(&peer2);
        let peer2_info = PeerInfo::new(peer2, PeerMeta { metadata: None });

        room_meta.on_kv_event(peers_map, MapEvent::OnSet(peer2_key, 0, peer2_info.serialize()));
        assert_eq!(room_meta.pop_output(Instant::now()), None);

        let owner = 1;
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: false },
            RoomInfoSubscribe { peers: true, tracks: false },
        );
        assert_eq!(
            room_meta.pop_output(Instant::now()),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::PeerJoined(peer2_info.peer.clone(), peer2_info.meta.clone())))
        );
        assert_eq!(room_meta.pop_output(Instant::now()), Some(Output::Kv(Control::MapCmd(peers_map, MapControl::Sub))));
        assert_eq!(room_meta.pop_output(Instant::now()), None);
    }

    //TODO Test join as track only => should subscribe only tracks, fire only track events
    #[test]
    fn join_track_only() {
        let room: ClusterRoomHash = 1.into();
        let peers_map = id_generator::peers_map(room);
        let tracks_map = id_generator::tracks_map(room);
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        let peer_info = PeerInfo::new(peer_id.clone(), peer_meta.clone());
        let peer_key = id_generator::peers_key(&peer_id);
        let owner = 1;
        let now = Instant::now();
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: true },
            RoomInfoSubscribe { peers: false, tracks: true },
        );
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(tracks_map, MapControl::Sub))));
        assert_eq!(room_meta.pop_output(now), None);

        // should handle incoming event with only track and reject peer
        room_meta.on_kv_event(peers_map, MapEvent::OnSet(peer_key, 0, peer_info.serialize()));
        assert_eq!(room_meta.pop_output(now), None);

        let track_name: TrackName = "audio_main".to_string().into();
        let track_info = TrackInfo::simple_audio(peer_id.clone());
        let track_key = id_generator::tracks_key(&peer_id, &track_name);
        room_meta.on_kv_event(tracks_map, MapEvent::OnSet(track_key, 0, track_info.serialize()));
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Endpoint(
                vec![owner],
                ClusterEndpointEvent::TrackStarted(peer_id.clone(), track_name.clone(), track_info.meta.clone())
            ))
        );
        assert_eq!(room_meta.pop_output(now), None);

        // should only handle remove track event, reject peer
        room_meta.on_kv_event(tracks_map, MapEvent::OnDel(track_key, 0));
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::TrackStopped(peer_id.clone(), track_name.clone(), track_info.meta)))
        );
        assert_eq!(room_meta.pop_output(now), None);

        room_meta.on_kv_event(peers_map, MapEvent::OnDel(peer_key, 0));
        assert_eq!(room_meta.pop_output(now), None);

        // peer leave should send unsub
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(tracks_map, MapControl::Unsub))));
        assert_eq!(room_meta.pop_output(now), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(now), None);
    }

    //join track only should restore old tracks
    #[test]
    fn join_sub_track_only_should_restore_old_tracks() {
        let room: ClusterRoomHash = 1.into();
        let tracks_map = id_generator::tracks_map(room);
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);

        let peer2: PeerId = "peer2".to_string().into();
        let track_name: TrackName = "audio_main".to_string().into();
        let track_key = id_generator::tracks_key(&peer2, &track_name);
        let track_info = TrackInfo::simple_audio(peer2);

        room_meta.on_kv_event(tracks_map, MapEvent::OnSet(track_key, 0, track_info.serialize()));
        assert_eq!(room_meta.pop_output(Instant::now()), None);

        let owner = 1;
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: false },
            RoomInfoSubscribe { peers: false, tracks: true },
        );
        assert_eq!(
            room_meta.pop_output(Instant::now()),
            Some(Output::Endpoint(
                vec![owner],
                ClusterEndpointEvent::TrackStarted(track_info.peer.clone(), track_info.track.clone(), track_info.meta.clone())
            ))
        );
        assert_eq!(room_meta.pop_output(Instant::now()), Some(Output::Kv(Control::MapCmd(tracks_map, MapControl::Sub))));
        assert_eq!(room_meta.pop_output(Instant::now()), None);
    }

    //Test manual no subscribe peer => dont fire any event
    #[test]
    fn join_manual_no_subscribe_peer() {
        let room: ClusterRoomHash = 1.into();
        let peers_map = id_generator::peers_map(room);
        let tracks_map = id_generator::tracks_map(room);
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        let peer_info = PeerInfo::new(peer_id.clone(), peer_meta.clone());
        let peer_key = id_generator::peers_key(&peer_id);
        let owner = 1;
        let now = Instant::now();
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: false },
            RoomInfoSubscribe { peers: false, tracks: false },
        );
        assert_eq!(room_meta.pop_output(now), None);

        // should handle incoming event with only track and reject peer
        room_meta.on_kv_event(peers_map, MapEvent::OnSet(peer_key, 0, peer_info.serialize()));
        assert_eq!(room_meta.pop_output(now), None);

        let track_name: TrackName = "audio_main".to_string().into();
        let track_info = TrackInfo::simple_audio(peer_id.clone());
        let track_key = id_generator::tracks_key(&peer_id, &track_name);
        room_meta.on_kv_event(tracks_map, MapEvent::OnSet(track_key, 0, track_info.serialize()));
        assert_eq!(room_meta.pop_output(now), None);

        // should only handle remove track event, reject peer
        room_meta.on_kv_event(tracks_map, MapEvent::OnDel(track_key, 0));
        assert_eq!(room_meta.pop_output(now), None);

        room_meta.on_kv_event(peers_map, MapEvent::OnDel(peer_key, 0));
        assert_eq!(room_meta.pop_output(now), None);

        // peer leave should send unsub
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(now), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(now), None);
    }

    //TODO Test manual and subscribe peer => should fire event
    #[test]
    fn join_manual_with_subscribe() {
        let room: ClusterRoomHash = 1.into();
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        let owner = 1;
        let now = Instant::now();
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: false },
            RoomInfoSubscribe { peers: false, tracks: false },
        );
        assert_eq!(room_meta.pop_output(now), None);

        let peer2: PeerId = "peer1".to_string().into();
        let peer2_map = id_generator::peer_map(room, &peer2);
        room_meta.on_subscribe_peer(owner, peer2.clone());
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(peer2_map, MapControl::Sub))));
        assert_eq!(room_meta.pop_output(now), None);

        // should handle incoming event with only track and reject peer
        let track_name: TrackName = "audio_main".to_string().into();
        let track_info = TrackInfo::simple_audio(peer_id.clone());
        let track_key = id_generator::tracks_key(&peer2, &track_name);
        room_meta.on_kv_event(peer2_map, MapEvent::OnSet(track_key, 0, track_info.serialize()));
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Endpoint(
                vec![owner],
                ClusterEndpointEvent::TrackStarted(peer2.clone(), track_name.clone(), track_info.meta.clone())
            ))
        );
        assert_eq!(room_meta.pop_output(now), None);

        // should only handle remove track event, reject peer
        room_meta.on_kv_event(peer2_map, MapEvent::OnDel(track_key, 0));
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::TrackStopped(peer2.clone(), track_name.clone(), track_info.meta)))
        );
        assert_eq!(room_meta.pop_output(now), None);

        // should send unsub when unsubscribe peer
        room_meta.on_unsubscribe_peer(owner, peer2.clone());
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(peer2_map, MapControl::Unsub))));
        assert_eq!(room_meta.pop_output(now), None);

        // peer leave should not send unsub
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(now), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(now), None);
    }

    //TODO Test track publish => should set key to both single peer map and tracks map
    #[test]
    fn track_publish_enable() {
        let room: ClusterRoomHash = 1.into();
        let tracks_map = id_generator::tracks_map(room);
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);

        let owner = 1;
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        let now = Instant::now();
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: true },
            RoomInfoSubscribe { peers: false, tracks: false },
        );
        assert_eq!(room_meta.pop_output(now), None);

        let track_id: RemoteTrackId = RemoteTrackId(1);
        let track_name: TrackName = "audio_main".to_string().into();
        let track_info = TrackInfo::simple_audio(peer_id.clone());
        let peer_map = id_generator::peer_map(room, &peer_id);
        let track_key = id_generator::tracks_key(&peer_id, &track_name);
        room_meta.on_track_publish(owner, track_id, track_name, track_info.meta.clone());
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Kv(Control::MapCmd(tracks_map, MapControl::Set(track_key, track_info.serialize()))))
        );
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Kv(Control::MapCmd(peer_map, MapControl::Set(track_key, track_info.serialize()))))
        );
        assert_eq!(room_meta.pop_output(now), None);

        //after unpublish should delete all tracks
        room_meta.on_track_unpublish(owner, track_id);
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(tracks_map, MapControl::Del(track_key)))));
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(peer_map, MapControl::Del(track_key)))));
        assert_eq!(room_meta.pop_output(now), None);

        //should not pop anything after leave
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(now), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(now), None);
    }

    //TODO Test track publish in disable mode => should not set key to both single peer map and tracks map
    #[test]
    fn track_publish_disable() {
        let room: ClusterRoomHash = 1.into();
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);

        let now = Instant::now();
        let owner = 1;
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: false },
            RoomInfoSubscribe { peers: false, tracks: false },
        );
        assert_eq!(room_meta.pop_output(now), None);

        let track_id: RemoteTrackId = RemoteTrackId(1);
        let track_name: TrackName = "audio_main".to_string().into();
        let track_info = TrackInfo::simple_audio(peer_id.clone());
        room_meta.on_track_publish(owner, track_id, track_name, track_info.meta.clone());
        assert_eq!(room_meta.pop_output(now), None);

        //after unpublish should delete all tracks
        room_meta.on_track_unpublish(owner, track_id);
        assert_eq!(room_meta.pop_output(now), None);

        //should not pop anything after leave
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(now), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(now), None);
    }

    /// Test leave room auto del remain remote tracks
    #[test]
    fn leave_room_auto_del_remote_tracks() {
        let room: ClusterRoomHash = 1.into();
        let tracks_map = id_generator::tracks_map(room);
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);

        let now = Instant::now();
        let owner = 1;
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: true },
            RoomInfoSubscribe { peers: false, tracks: false },
        );
        assert_eq!(room_meta.pop_output(now), None);

        let track_id: RemoteTrackId = RemoteTrackId(1);
        let track_name: TrackName = "audio_main".to_string().into();
        let track_info = TrackInfo::simple_audio(peer_id.clone());
        let peer_map = id_generator::peer_map(room, &peer_id);
        let track_key = id_generator::tracks_key(&peer_id, &track_name);
        room_meta.on_track_publish(owner, track_id, track_name, track_info.meta.clone());
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Kv(Control::MapCmd(tracks_map, MapControl::Set(track_key, track_info.serialize()))))
        );
        assert_eq!(
            room_meta.pop_output(now),
            Some(Output::Kv(Control::MapCmd(peer_map, MapControl::Set(track_key, track_info.serialize()))))
        );
        assert_eq!(room_meta.pop_output(now), None);

        //after leave should auto delete all tracks
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(tracks_map, MapControl::Del(track_key)))));
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(peer_map, MapControl::Del(track_key)))));
        assert_eq!(room_meta.pop_output(now), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(now), None);
    }

    // Leave room auto unsub private peer maps
    #[test]
    fn leave_room_auto_unsub_private_peer_maps() {
        let room: ClusterRoomHash = 1.into();
        let mut room_meta: RoomMetadata<u8> = RoomMetadata::<u8>::new(room);
        let peer_id: PeerId = "peer1".to_string().into();
        let peer_meta = PeerMeta { metadata: None };
        let owner = 1;
        let now = Instant::now();
        room_meta.on_join(
            owner,
            peer_id.clone(),
            peer_meta.clone(),
            RoomInfoPublish { peer: false, tracks: false },
            RoomInfoSubscribe { peers: false, tracks: false },
        );
        assert_eq!(room_meta.pop_output(now), None);

        let peer2: PeerId = "peer1".to_string().into();
        let peer2_map = id_generator::peer_map(room, &peer2);
        room_meta.on_subscribe_peer(owner, peer2.clone());
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(peer2_map, MapControl::Sub))));
        assert_eq!(room_meta.pop_output(now), None);

        // peer leave should send unsub of peer2_map
        room_meta.on_leave(owner);
        assert_eq!(room_meta.pop_output(now), Some(Output::Kv(Control::MapCmd(peer2_map, MapControl::Unsub))));
        assert_eq!(room_meta.pop_output(now), Some(Output::LastPeerLeaved));
        assert_eq!(room_meta.pop_output(now), None);
    }
}
