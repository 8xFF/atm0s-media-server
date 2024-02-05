use atm0s_sdn::NodeAddr;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

mod endpoint;
mod local_track;
mod media;
mod remote_track;
pub mod rpc;
mod secure;
mod session_uuid;

pub use endpoint::*;
pub use local_track::*;
pub use media::*;
pub use remote_track::*;
pub use secure::*;
pub use session_uuid::*;

pub type ClusterPeerId = String;
pub type ClusterTrackName = String;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct ClusterTrackUuid(u32);

impl ClusterTrackUuid {
    pub fn from_info(room_id: &str, peer_id: &str, track_name: &str) -> Self {
        let based = format!("{}-{}-{}", room_id, peer_id, track_name);
        let mut s = DefaultHasher::new();
        based.hash(&mut s);
        Self(s.finish() as u32)
    }
}

impl Deref for ClusterTrackUuid {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<u32> for ClusterTrackUuid {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClusterEndpointError {
    InternalError,
}

pub trait Cluster<C>: Send + Sync
where
    C: ClusterEndpoint,
{
    fn node_id(&self) -> u32;
    fn node_addr(&self) -> NodeAddr;
    fn build(&mut self, room_id: &str, peer_id: &str) -> C;
}

pub const GATEWAY_SERVICE: u8 = 101;
pub const MEDIA_SERVER_SERVICE: u8 = 102;
pub const CONNECTOR_SERVICE: u8 = 103;
