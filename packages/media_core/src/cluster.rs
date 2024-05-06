//! Cluster handle all of logic allow multi node can collaborate to make a giant streaming system.
//!
//! Cluster is collect of some rooms, each room is independent logic.
//! We use UserData feature from SDN with UserData is ClusterRoomHash to route SDN event to correct room.
//!

use derive_more::{AsRef, Display, From};
use sans_io_runtime::TaskGroup;
use std::{
    collections::HashMap,
    fmt::Debug,
    hash::{Hash, Hasher},
    time::Instant,
};

use atm0s_sdn::features::{FeaturesControl, FeaturesEvent};
use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackName},
    media::MediaPacket,
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
    PeerLeaved(PeerId),
    TrackStarted(PeerId, TrackName, TrackMeta),
    TrackStopped(PeerId, TrackName),
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
    rooms: TaskGroup<room::Input<Owner>, room::Output<Owner>, ClusterRoom<Owner>, 128>,
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
    pub fn on_tick(&mut self, now: Instant) -> Option<Output<Owner>> {
        let (index, out) = self.rooms.on_tick(now)?;
        Some(self.process_room_output(index, out))
    }

    pub fn on_sdn_event(&mut self, now: Instant, room: ClusterRoomHash, event: FeaturesEvent) -> Option<Output<Owner>> {
        let index = self.rooms_map.get(&room)?;
        let out = self.rooms.on_event(now, *index, room::Input::Sdn(event))?;
        Some(self.process_room_output(*index, out))
    }

    pub fn on_endpoint_control(&mut self, now: Instant, owner: Owner, room_hash: ClusterRoomHash, control: ClusterEndpointControl) -> Option<Output<Owner>> {
        if let Some(index) = self.rooms_map.get(&room_hash) {
            let out = self.rooms.on_event(now, *index, room::Input::Endpoint(owner, control))?;
            Some(self.process_room_output(*index, out))
        } else {
            log::info!("[MediaCluster] create room {}", room_hash);
            let index = self.rooms.add_task(ClusterRoom::new(room_hash));
            self.rooms_map.insert(room_hash, index);
            let out = self.rooms.on_event(now, index, room::Input::Endpoint(owner, control))?;
            Some(self.process_room_output(index, out))
        }
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        let (index, out) = self.rooms.pop_output(now)?;
        Some(self.process_room_output(index, out))
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<Owner>> {
        let (index, out) = self.rooms.shutdown(now)?;
        Some(self.process_room_output(index, out))
    }

    fn process_room_output(&mut self, index: usize, out: room::Output<Owner>) -> Output<Owner> {
        match out {
            room::Output::Sdn(userdata, control) => Output::Sdn(userdata, control),
            room::Output::Endpoint(owners, event) => Output::Endpoint(owners, event),
            room::Output::Destroy(room) => {
                log::info!("[MediaCluster] remove room index {index}, hash {room}");
                self.rooms_map.remove(&room).expect("Should have room with index");
                self.rooms.remove_task(index);
                Output::Continue
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

        // Not join room with scope (peer true, track false) should Set and Sub
        let out = cluster.on_endpoint_control(
            Instant::now(),
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
            out,
            Some(Output::Sdn(
                room_hash,
                FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Set(peer_key, peer_info.serialize())))
            ))
        );
        assert_eq!(
            cluster.pop_output(Instant::now()),
            Some(Output::Sdn(room_hash, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Sub))))
        );
        assert_eq!(cluster.pop_output(Instant::now()), None);
        assert_eq!(cluster.rooms.tasks(), 1);
        assert_eq!(cluster.rooms_map.len(), 1);

        // Correct forward to room
        let out = cluster.on_sdn_event(
            Instant::now(),
            room_hash,
            FeaturesEvent::DhtKv(dht_kv::Event::MapEvent(room_peers_map, MapEvent::OnSet(peer_key, 1, peer_info.serialize()))),
        );
        assert_eq!(out, Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::PeerJoined(peer.clone(), peer_info.meta.clone()))));
        assert_eq!(cluster.pop_output(Instant::now()), None);

        // Now leave room should Del and Unsub
        let out = cluster.on_endpoint_control(Instant::now(), owner, room_hash, ClusterEndpointControl::Leave);
        assert_eq!(
            out,
            Some(Output::Sdn(room_hash, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Del(peer_key)))))
        );
        assert_eq!(
            cluster.pop_output(Instant::now()),
            Some(Output::Sdn(room_hash, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(room_peers_map, MapControl::Unsub))))
        );
        assert_eq!(cluster.pop_output(Instant::now()), Some(Output::Continue)); //this is for destroy event
        assert_eq!(cluster.pop_output(Instant::now()), None);
        assert_eq!(cluster.rooms.tasks(), 0);
        assert_eq!(cluster.rooms_map.len(), 0);
    }
}
