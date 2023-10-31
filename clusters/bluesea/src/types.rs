use bytes::Bytes;
use cluster::{ClusterTrackMeta, ClusterTrackStats};
use media_utils::hash_str;
use serde::{Deserialize, Serialize};
use transport::MediaPacket;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RoomStream {
    peer: String,
    track: String,
    meta: ClusterTrackMeta,
}

pub fn to_room_key(peer: &str, track: &str) -> u64 {
    hash_str(&format!("{peer}/{track}"))
}

pub fn to_room_value(peer: &str, track: &str, meta: ClusterTrackMeta) -> (u64, Vec<u8>) {
    (
        hash_str(&format!("{peer}/{track}")),
        bincode::serialize(&RoomStream {
            peer: peer.to_string(),
            track: track.to_string(),
            meta,
        })
        .expect("should serialize")
        .to_vec(),
    )
}

pub fn from_room_value(key: u64, data: &[u8]) -> Option<(String, String, ClusterTrackMeta)> {
    let data = bincode::deserialize::<RoomStream>(data).ok()?;
    if key == hash_str(&format!("{}/{}", data.peer, data.track)) {
        Some((data.peer, data.track, data.meta))
    } else {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub enum TrackData {
    Media(MediaPacket),
    Stats(ClusterTrackStats),
}

impl TryFrom<Bytes> for TrackData {
    type Error = ();

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        //TODO avoid using bincode here
        bincode::deserialize(&value[..]).map_err(|_| ())
    }
}

impl TryInto<Bytes> for TrackData {
    type Error = ();

    fn try_into(self) -> Result<Bytes, Self::Error> {
        //TODO avoid using bincode here
        bincode::serialize(&self).map(|v| v.into()).map_err(|_| ())
    }
}
