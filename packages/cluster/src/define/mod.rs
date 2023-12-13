use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

mod endpoint;
mod local_track;
mod media;
mod remote_track;
pub mod rpc;

pub use endpoint::*;
pub use local_track::*;
pub use media::*;
pub use remote_track::*;

pub type ClusterTrackUuid = u64;
pub type ClusterPeerId = String;
pub type ClusterTrackName = String;

#[derive(Debug, PartialEq, Eq)]
pub enum ClusterEndpointError {
    InternalError,
}

/// generate for other peer
pub fn generate_cluster_track_uuid(room_id: &str, peer_id: &str, track_name: &str) -> ClusterTrackUuid {
    let based = format!("{}-{}-{}", room_id, peer_id, track_name);
    let mut s = DefaultHasher::new();
    based.hash(&mut s);
    s.finish()
}

pub trait Cluster<C>: Send + Sync
where
    C: ClusterEndpoint,
{
    fn node_id(&self) -> u32;
    fn build(&mut self, room_id: &str, peer_id: &str) -> C;
}

pub const GLOBAL_GATEWAY_SERVICE: u8 = 100;
pub const INNER_GATEWAY_SERVICE: u8 = 101;
pub const MEDIA_SERVER_SERVICE: u8 = 102;
pub const CONNECTOR_SERVICE: u8 = 103;
