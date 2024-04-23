//! Cluster handle all of logic allow multi node can collaborate to make a giant streaming system.
//!
//! Cluster is collect of some rooms, each room is independent logic.
//! We use UserData feature from SDN with UserData is ClusterRoomHash to route SDN event to correct room.
//!

use derive_more::{AsRef, Display, From};
use sans_io_runtime::{Task, TaskGroup};
use std::{
    collections::HashMap,
    fmt::Debug,
    hash::{Hash, Hasher},
    time::Instant,
};

use atm0s_sdn::features::{FeaturesControl, FeaturesEvent};
use media_server_protocol::{
    endpoint::{PeerId, RoomId, TrackMeta, TrackName},
    media::MediaPacket,
};

use crate::transport::{LocalTrackId, RemoteTrackId};

use self::room::ClusterRoom;

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

#[derive(Debug, Clone)]
pub enum ClusterRemoteTrackControl {
    Started(TrackName, TrackMeta),
    Media(MediaPacket),
    Ended,
}

#[derive(Clone)]
pub enum ClusterRemoteTrackEvent {
    RequestKeyFrame,
    LimitBitrate { min: u32, max: u32 },
}

#[derive(Debug, Clone)]
pub enum ClusterLocalTrackControl {
    Subscribe(PeerId, TrackName),
    RequestKeyFrame,
    DesiredBitrate(u32),
    Unsubscribe,
}

#[derive(Debug, Clone)]
pub enum ClusterLocalTrackEvent {
    Started,
    SourceChanged,
    Media(MediaPacket),
    Ended,
}

#[derive(Debug)]
pub enum ClusterRoomInfoPublishLevel {
    Full,
    TrackOnly,
}

#[derive(Debug)]
pub enum ClusterRoomInfoSubscribeLevel {
    Full,
    TrackOnly,
    Manual,
}

#[derive(Debug)]
pub enum ClusterEndpointControl {
    Join(PeerId, ClusterRoomInfoPublishLevel, ClusterRoomInfoSubscribeLevel),
    Leave,
    SubscribePeer(PeerId),
    UnsubscribePeer(PeerId),
    RemoteTrack(RemoteTrackId, ClusterRemoteTrackControl),
    LocalTrack(LocalTrackId, ClusterLocalTrackControl),
}

#[derive(Clone)]
pub enum ClusterEndpointEvent {
    TrackStarted(PeerId, TrackName, TrackMeta),
    TrackStoped(PeerId, TrackName),
    RemoteTrack(RemoteTrackId, ClusterRemoteTrackEvent),
    LocalTrack(LocalTrackId, ClusterLocalTrackEvent),
}

pub enum Input<Owner> {
    Sdn(ClusterRoomHash, FeaturesEvent),
    Endpoint(Owner, ClusterRoomHash, ClusterEndpointControl),
}

pub enum Output<Owner> {
    Sdn(ClusterRoomHash, FeaturesControl),
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    Continue,
}

pub struct MediaCluster<Owner: Debug + Copy + Clone + Hash + Eq> {
    rooms_map: HashMap<ClusterRoomHash, usize>,
    rooms: TaskGroup<room::Input<Owner>, Output<Owner>, ClusterRoom<Owner>, 128>,
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
        let (_index, out) = self.rooms.on_tick(now)?;
        Some(out)
    }

    pub fn on_sdn_event(&mut self, now: Instant, room: ClusterRoomHash, event: FeaturesEvent) -> Option<Output<Owner>> {
        let index = self.rooms_map.get(&room)?;
        self.rooms.on_event(now, *index, room::Input::Sdn(event))
    }

    pub fn on_endpoint_control(&mut self, now: Instant, owner: Owner, room_hash: ClusterRoomHash, control: ClusterEndpointControl) -> Option<Output<Owner>> {
        if let Some(index) = self.rooms_map.get(&room_hash) {
            self.rooms.on_event(now, *index, room::Input::Endpoint(owner, control))
        } else {
            log::info!("[MediaCluster] create room {}", room_hash);
            let mut room = ClusterRoom::new(room_hash);
            let out = room.on_event(now, room::Input::Endpoint(owner, control));
            let index = self.rooms.add_task(room);
            self.rooms_map.insert(room_hash, index);
            out
        }
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        let (_index, out) = self.rooms.pop_output(now)?;
        Some(out)
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<Owner>> {
        let (_index, out) = self.rooms.shutdown(now)?;
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    //TODO should create room when new room event arrived
    //TODO should route to correct room
}
