//! Cluster handle all of logic allow multi node can collaborate to make a giant streaming system.
//!
//! Cluster is collect of some rooms, each room is independent logic.
//! We use UserData feature from SDN with UserData is ClusterRoomHash to route SDN event to correct room.
//!

use derive_more::{AsRef, Display, From};
use sans_io_runtime::{return_if_none, TaskGroup, TaskSwitcherChild};
use std::{
    collections::HashMap,
    fmt::Debug,
    hash::{Hash, Hasher},
    time::Instant,
};

use atm0s_sdn::features::{FeaturesControl, FeaturesEvent};
use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackName},
    media::{MediaKind, MediaPacket},
};

use crate::transport::{LocalTrackId, RemoteTrackId};

use self::room::ClusterRoom;

mod id_generator;
mod room;

#[derive(Clone, Copy, From, AsRef, PartialEq, Eq, Debug, Display, Hash)]
pub struct ClusterRoomHash(pub u64);

impl From<&RoomId> for ClusterRoomHash {
    fn from(room: &RoomId) -> Self {
        let mut hash = std::hash::DefaultHasher::new();
        room.as_ref().hash(&mut hash);
        Self(hash.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterRemoteTrackControl {
    Started(TrackName, TrackMeta),
    Media(MediaPacket),
    Ended,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterRemoteTrackEvent {
    RequestKeyFrame,
    LimitBitrate { min: u64, max: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterLocalTrackControl {
    Subscribe(PeerId, TrackName),
    RequestKeyFrame,
    DesiredBitrate(u64),
    Unsubscribe,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterLocalTrackEvent {
    Started,
    SourceChanged,
    Media(u64, MediaPacket),
    Ended,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClusterEndpointControl {
    Join(PeerId, PeerMeta, RoomInfoPublish, RoomInfoSubscribe),
    Leave,
    SubscribePeer(PeerId),
    UnsubscribePeer(PeerId),
    RemoteTrack(RemoteTrackId, ClusterRemoteTrackControl),
    LocalTrack(LocalTrackId, ClusterLocalTrackControl),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterEndpointEvent {
    PeerJoined(PeerId, PeerMeta),
    PeerLeaved(PeerId, PeerMeta),
    TrackStarted(PeerId, TrackName, TrackMeta),
    TrackStopped(PeerId, TrackName, TrackMeta),
    RemoteTrack(RemoteTrackId, ClusterRemoteTrackEvent),
    LocalTrack(LocalTrackId, ClusterLocalTrackEvent),
}

pub enum Input<Owner> {
    Sdn(ClusterRoomHash, FeaturesEvent),
    Endpoint(Owner, ClusterRoomHash, ClusterEndpointControl),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output<Owner> {
    Sdn(ClusterRoomHash, FeaturesControl),
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    Continue,
}

pub struct MediaCluster<Owner: Debug + Copy + Clone + Hash + Eq> {
    rooms_map: HashMap<ClusterRoomHash, usize>,
    rooms: TaskGroup<room::Input<Owner>, room::Output<Owner>, ClusterRoom<Owner>, 16>,
}

impl<Owner: Debug + Copy + Hash + Eq + Clone> Default for MediaCluster<Owner> {
    fn default() -> Self {
        Self {
            rooms_map: HashMap::new(),
            rooms: TaskGroup::default(),
        }
    }
}

impl<Owner: Debug + Hash + Copy + Clone + Debug + Eq> MediaCluster<Owner> {
    pub fn on_tick(&mut self, now: Instant) {
        self.rooms.on_tick(now);
    }

    pub fn on_sdn_event(&mut self, now: Instant, room: ClusterRoomHash, event: FeaturesEvent) {
        let index = return_if_none!(self.rooms_map.get(&room));
        self.rooms.on_event(now, *index, room::Input::Sdn(event));
    }

    pub fn on_endpoint_control(&mut self, now: Instant, owner: Owner, room_hash: ClusterRoomHash, control: ClusterEndpointControl) {
        if let Some(index) = self.rooms_map.get(&room_hash) {
            self.rooms.on_event(now, *index, room::Input::Endpoint(owner, control));
        } else {
            log::info!("[MediaCluster] create room {}", room_hash);
            let index = self.rooms.add_task(ClusterRoom::new(room_hash));
            self.rooms_map.insert(room_hash, index);
            self.rooms.on_event(now, index, room::Input::Endpoint(owner, control));
        }
    }

    pub fn shutdown<'a>(&mut self, now: Instant) {
        self.rooms.on_shutdown(now);
    }
}

impl<Owner: Debug + Hash + Copy + Clone + Debug + Eq> TaskSwitcherChild<Output<Owner>> for MediaCluster<Owner> {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        let (index, out) = self.rooms.pop_output(now)?;
        match out {
            room::Output::Sdn(userdata, control) => Some(Output::Sdn(userdata, control)),
            room::Output::Endpoint(owners, event) => Some(Output::Endpoint(owners, event)),
            room::Output::Destroy(room) => {
                log::info!("[MediaCluster] remove room index {index}, hash {room}");
                self.rooms_map.remove(&room).expect("Should have room with index");
                self.rooms.remove_task(index);
                Some(Output::Continue)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use atm0s_sdn::features::{
        dht_kv::{self, MapControl, MapEvent},
        FeaturesControl, FeaturesEvent,
    };
    use media_server_protocol::endpoint::{PeerId, PeerInfo, PeerMeta, RoomInfoPublish, RoomInfoSubscribe};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::cluster::{id_generator, ClusterEndpointEvent};

    use super::{ClusterEndpointControl, ClusterRoomHash, MediaCluster, Output};

    //TODO should create room when new room event arrived
    //TODO should route to correct room
    //TODO should remove room after all peers leaved
    #[test]
    fn room_manager_should_work() {
        let mut cluster = MediaCluster::<u8>::default();

        let owner = 1;
        let room_hash = ClusterRoomHash(1);
        let room_peers_map = id_generator::peers_map(room_hash);
        let peer = PeerId("peer1".to_string());
        let peer_key = id_generator::peers_key(&peer);
        let peer_info = PeerInfo::new(peer.clone(), PeerMeta { metadata: None });

        let now = Instant::now();
        // Not join room with scope (peer true, track false) should Set and Sub
        cluster.on_endpoint_control(
            now,
            owner,
            room_hash,
            ClusterEndpointControl::Join(
                peer.clone(),
                peer_info.meta.clone(),
                RoomInfoPublish { peer: true, tracks: false },
                RoomInfoSubscribe { peers: true, tracks: false },
            ),
        );
        assert_eq!(
            cluster.pop_output(now),
            Some(Output::Sdn(
                room_hash,
                FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Set(peer_key, peer_info.serialize())))
            ))
        );
        assert_eq!(
            cluster.pop_output(now),
            Some(Output::Sdn(room_hash, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Sub))))
        );
        assert_eq!(cluster.pop_output(now), None);
        assert_eq!(cluster.rooms.tasks(), 1);
        assert_eq!(cluster.rooms_map.len(), 1);

        // Correct forward to room
        cluster.on_sdn_event(
            now,
            room_hash,
            FeaturesEvent::DhtKv(dht_kv::Event::MapEvent(room_peers_map, MapEvent::OnSet(peer_key, 1, peer_info.serialize()))),
        );
        assert_eq!(
            cluster.pop_output(now),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::PeerJoined(peer.clone(), peer_info.meta.clone())))
        );
        assert_eq!(cluster.pop_output(now), None);

        // Now leave room should Del and Unsub
        cluster.on_endpoint_control(now, owner, room_hash, ClusterEndpointControl::Leave);
        assert_eq!(
            cluster.pop_output(now),
            Some(Output::Sdn(room_hash, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Del(peer_key)))))
        );
        assert_eq!(
            cluster.pop_output(now),
            Some(Output::Sdn(room_hash, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Unsub))))
        );
        assert_eq!(cluster.pop_output(now), Some(Output::Continue)); //this is for destroy event
        assert_eq!(cluster.pop_output(now), None);
        assert_eq!(cluster.rooms.tasks(), 0);
        assert_eq!(cluster.rooms_map.len(), 0);
    }
}
