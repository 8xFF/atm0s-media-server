use std::hash::{DefaultHasher, Hash, Hasher};

use atm0s_sdn::features::dht_kv::{Key, Map};
use media_server_protocol::endpoint::{PeerId, TrackName};

use super::ClusterRoomHash;

pub fn peer_map(room: ClusterRoomHash, peer: &PeerId) -> Map {
    let mut h = DefaultHasher::new();
    room.as_ref().hash(&mut h);
    peer.as_ref().hash(&mut h);
    h.finish().into()
}

pub fn peers_map(room: ClusterRoomHash) -> Map {
    room.0.into()
}

pub fn peers_key(peer: &PeerId) -> Key {
    let mut h = DefaultHasher::new();
    peer.as_ref().hash(&mut h);
    h.finish().into()
}

pub fn tracks_map(room: ClusterRoomHash) -> Map {
    (room.0 + 1).into()
}

pub fn tracks_key(peer: &PeerId, track: &TrackName) -> Key {
    let mut h = DefaultHasher::new();
    peer.as_ref().hash(&mut h);
    track.as_ref().hash(&mut h);
    h.finish().into()
}

pub fn gen_channel_id<T: From<u64>>(room: ClusterRoomHash, peer: &PeerId, track: &TrackName) -> T {
    let mut h = std::hash::DefaultHasher::new();
    room.as_ref().hash(&mut h);
    peer.as_ref().hash(&mut h);
    track.as_ref().hash(&mut h);
    h.finish().into()
}
