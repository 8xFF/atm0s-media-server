use crate::{ClusterEndpointMeta, ClusterTrackMeta, ClusterTrackStats};
use bytes::Bytes;
use media_utils::hash_str;
use serde::{Deserialize, Serialize};
use transport::MediaPacket;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RoomPeer {
    peer: String,
    meta: ClusterEndpointMeta,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RoomStream {
    peer: String,
    track: String,
    meta: ClusterTrackMeta,
}

pub fn to_room_peers_map_key(room: &str) -> u64 {
    hash_str(&format!("{room}/peers"))
}

pub fn to_room_streams_map_key(room: &str) -> u64 {
    hash_str(&format!("{room}/streams"))
}

pub fn to_peer_sub_key(peer: &str) -> u64 {
    hash_str(peer)
}

pub fn to_peer_value(peer: &str, meta: ClusterEndpointMeta) -> (u64, Vec<u8>) {
    (
        to_peer_sub_key(peer),
        bincode::serialize(&RoomPeer { peer: peer.to_string(), meta }).expect("should serialize").to_vec(),
    )
}

pub fn from_peer_value(key: u64, data: &[u8]) -> Option<(String, ClusterEndpointMeta)> {
    let data = bincode::deserialize::<RoomPeer>(data).ok()?;
    if key == hash_str(&data.peer) {
        Some((data.peer, data.meta))
    } else {
        None
    }
}

pub fn to_stream_sub_key(peer: &str, track: &str) -> u64 {
    hash_str(&format!("{peer}/{track}"))
}

pub fn to_stream_value(peer: &str, track: &str, meta: ClusterTrackMeta) -> (u64, Vec<u8>) {
    (
        to_stream_sub_key(peer, track),
        bincode::serialize(&RoomStream {
            peer: peer.to_string(),
            track: track.to_string(),
            meta,
        })
        .expect("should serialize")
        .to_vec(),
    )
}

pub fn from_stream_value(key: u64, data: &[u8]) -> Option<(String, String, ClusterTrackMeta)> {
    let data = bincode::deserialize::<RoomStream>(data).ok()?;
    if key == hash_str(&format!("{}/{}", data.peer, data.track)) {
        Some((data.peer, data.track, data.meta))
    } else {
        None
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_room_key() {
        let peer = "peer1";
        let track = "track1";
        let expected = 0x251D560B3DE7BBFF;
        assert_eq!(to_stream_sub_key(peer, track), expected);
    }

    #[test]
    fn test_to_room_value() {
        let peer = "peer1";
        let track = "track1";
        let meta = ClusterTrackMeta::default_audio();
        let expected_key = 0x251D560B3DE7BBFF;
        let expected_value = bincode::serialize(&RoomStream {
            peer: peer.to_string(),
            track: track.to_string(),
            meta: meta.clone(),
        })
        .expect("should serialize")
        .to_vec();
        assert_eq!(to_stream_value(peer, track, meta), (expected_key, expected_value));
    }

    #[test]
    fn test_from_room_value() {
        let peer = "peer1";
        let track = "track1";
        let meta = ClusterTrackMeta::default_audio();
        let expected = Some((peer.to_string(), track.to_string(), meta.clone()));
        let key = 0x251D560B3DE7BBFF;
        let value = bincode::serialize(&RoomStream {
            peer: peer.to_string(),
            track: track.to_string(),
            meta,
        })
        .expect("should serialize")
        .to_vec();
        assert_eq!(from_stream_value(key, &value), expected);
    }

    #[test]
    fn test_try_from_bytes() {
        let media_packet = MediaPacket::default();
        let track_data = TrackData::Media(media_packet.clone());
        let bytes = Bytes::from(bincode::serialize(&track_data).unwrap());
        assert_eq!(TrackData::try_from(bytes).unwrap(), track_data);
    }

    #[test]
    fn test_try_into_bytes() {
        let media_packet = MediaPacket::default();
        let track_data = TrackData::Media(media_packet.clone());
        let expected = Bytes::from(bincode::serialize(&track_data).unwrap());
        assert_eq!(TryInto::<Bytes>::try_into(track_data).unwrap(), expected);
    }

    //TODO test peer data
}
