//!
//! Medata part takecare of how cluster will store peer, track info.
//! We have 3 level: Full, Track only and Manual
//!
//! - Full: subscribe on both peer and track infomation
//! - Track only: subscribe on track info, this method is useful with large users application like broadcast or webinar
//! - Manual: client manual call subscribe on which peer it interested in, this method is useful with some spartial audio application
//!

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    time::Instant,
};

use atm0s_sdn::features::dht_kv::{self, Key, Map, MapControl, MapEvent};
use media_server_protocol::{
    endpoint::{PeerId, TrackMeta, TrackName},
    media::TrackInfo,
};

use crate::{
    cluster::{ClusterEndpointEvent, ClusterRoomHash, ClusterRoomInfoPublishLevel, ClusterRoomInfoSubscribeLevel},
    transport::RemoteTrackId,
};

pub enum Output<Owner> {
    Kv(dht_kv::Control),
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
}

pub struct RoomMetadata<Owner> {
    room: ClusterRoomHash,
    room_map: Map,
    peers: HashMap<Owner, PeerId>,
    local_tracks: HashMap<(Owner, RemoteTrackId), (PeerId, TrackName, Key)>,
    remote_tracks: HashMap<dht_kv::Key, (PeerId, TrackName, TrackMeta)>,
    queue: VecDeque<Output<Owner>>,
}

impl<Owner: Hash + Eq + Copy + Debug> RoomMetadata<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            room_map: room.0.into(),
            peers: HashMap::new(),
            local_tracks: HashMap::new(),
            remote_tracks: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn get_peer_from_owner(&self, owner: Owner) -> Option<PeerId> {
        self.peers.get(&owner).cloned()
    }

    pub fn on_join(&mut self, owner: Owner, peer: PeerId, publish: ClusterRoomInfoPublishLevel, subscribe: ClusterRoomInfoSubscribeLevel) -> Option<Output<Owner>> {
        log::info!("[ClusterRoom {}] join peer ({peer})", self.room);
        self.peers.insert(owner.clone(), peer);
        if self.peers.len() == 1 {
            log::info!("[ClusterRoom {}] first peer join => subscribe room map", self.room);
            Some(Output::Kv(dht_kv::Control::MapCmd(self.room_map, MapControl::Sub)))
        } else {
            log::info!("[ClusterRoom {}] next peer join => restore {} remote tracks", self.room, self.remote_tracks.len());
            for (_track_key, (peer, name, meta)) in &self.remote_tracks {
                self.queue
                    .push_back(Output::Endpoint(vec![owner.clone()], ClusterEndpointEvent::TrackStarted(peer.clone(), name.clone(), meta.clone())));
            }

            self.queue.pop_front()
        }
    }

    pub fn on_leave(&mut self, owner: Owner) -> Option<Output<Owner>> {
        let peer = self.peers.remove(&owner).expect("Should have owner");
        log::info!("[ClusterRoom {}] leave peer ({peer})", self.room);
        if self.peers.is_empty() {
            log::info!("[ClusterRoom {}] last peer leave => unsubscribe room map", self.room);
            Some(Output::Kv(dht_kv::Control::MapCmd(self.room_map, MapControl::Unsub)))
        } else {
            None
        }
    }

    pub fn on_subscribe_peer(&mut self, owner: Owner, target: PeerId) -> Option<Output<Owner>> {
        todo!()
    }

    pub fn on_unsubscribe_peer(&mut self, owner: Owner, target: PeerId) -> Option<Output<Owner>> {
        todo!()
    }

    pub fn on_track_publish(&mut self, owner: Owner, track_id: RemoteTrackId, track: TrackName, meta: TrackMeta) -> Option<Output<Owner>> {
        let peer = self.peers.get(&owner)?;
        let info = TrackInfo {
            peer: peer.clone(),
            track: track.clone(),
            meta,
        };
        let map_key = super::track_key(self.room, &peer, &track);
        self.local_tracks.insert((owner, track_id), (peer.clone(), track.clone(), map_key));
        Some(Output::Kv(dht_kv::Control::MapCmd((*self.room.as_ref()).into(), MapControl::Set(map_key, info.serialize()))))
    }

    pub fn on_track_unpublish(&mut self, owner: Owner, track_id: RemoteTrackId) -> Option<Output<Owner>> {
        let (peer, track, map_key) = self.local_tracks.remove(&(owner, track_id))?;
        Some(Output::Kv(dht_kv::Control::MapCmd((*self.room.as_ref()).into(), MapControl::Del(map_key))))
    }

    pub fn on_kv_event(&mut self, map: Map, event: MapEvent) -> Option<Output<Owner>> {
        if self.room_map == map {
            match event {
                dht_kv::MapEvent::OnSet(track_key, _source, data) => self.on_room_kv_event(track_key, Some(data)),
                dht_kv::MapEvent::OnDel(track_key, _source) => self.on_room_kv_event(track_key, None),
                dht_kv::MapEvent::OnRelaySelected(_) => None,
            }
        } else {
            None
        }
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }

    fn on_room_kv_event(&mut self, track: dht_kv::Key, data: Option<Vec<u8>>) -> Option<Output<Owner>> {
        let info = if let Some(data) = data {
            Some(TrackInfo::deserialize(&data)?)
        } else {
            None
        };

        let peers = self.peers.keys().cloned().collect::<Vec<_>>();
        if let Some(info) = info {
            log::info!("[ClusterRoom {}] cluster: peer ({}) started track {}) => fire event to {:?}", self.room, info.peer, info.track, peers);
            self.remote_tracks.insert(track, (info.peer.clone(), info.track.clone(), info.meta.clone()));
            Some(Output::Endpoint(peers, ClusterEndpointEvent::TrackStarted(info.peer, info.track, info.meta)))
        } else {
            let (peer, name, _meta) = self.remote_tracks.remove(&track)?;
            log::info!("[ClusterRoom {}] cluster: peer ({}) stopped track {}) => fire event to {:?}", self.room, peer, name, peers);
            Some(Output::Endpoint(peers, ClusterEndpointEvent::TrackStoped(peer, name)))
        }
    }
}

#[cfg(test)]
mod tests {
    //TODO Test join as full => should subscribe both peers and tracks, fire both peer and track events
    //TODO Test leave as full => should unsubscribe both peers and tracks
    //TODO Test join as track only => should subscribe only tracks, fire only track events
    //TODO Test leave as track only => should unsubscribe only tracks
    //TODO Test join as manual => dont subscribe
    //TODO Test leave as manual => don unsubscribe
    //TODO Test manual and subscribe peer => should subscribe that peer
    //TODO Test manual and unsubscribe peer => should unsubscribe that peer
    //TODO Test track publish => should set key to both single peer map and tracks map
    //TODO Test track unpublish => should del key to both single peer map and tracks map
    //TODO Handle kv event => should handle peers map
    //TODO Handle kv event => should handle tracks map
    //TODO Handle kv event => should handle single peer map
}
