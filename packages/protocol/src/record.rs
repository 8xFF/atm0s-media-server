use serde::{Deserialize, Serialize};

use crate::{
    endpoint::{PeerId, RoomId, TrackMeta, TrackName},
    media::MediaPacket,
    multi_tenancy::AppId,
    transport::RemoteTrackId,
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SessionRecordHeader {
    pub room: String,
    pub peer: String,
    pub session: u64,
    pub start_ts: u64,
    pub end_ts: u64,
}

impl SessionRecordHeader {
    pub fn write_to(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = bincode::serialized_size(self).expect("Should calc bincode_size") as usize;
        if len > buf.len() {
            Err(std::io::Error::new(std::io::ErrorKind::OutOfMemory, "Buffer too small"))
        } else {
            bincode::serialize_into(buf, self).expect("Should serialize ok");
            Ok(len)
        }
    }

    pub fn read_from(buf: &[u8]) -> std::io::Result<Self> {
        bincode::deserialize(buf).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bincode deserialize error"))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum SessionRecordEvent {
    JoinRoom(AppId, RoomId, PeerId),
    LeaveRoom,
    TrackStarted(RemoteTrackId, TrackName, TrackMeta),
    TrackStopped(RemoteTrackId),
    TrackMedia(RemoteTrackId, MediaPacket),
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecordRow {
    pub ts: u64,
    pub event: SessionRecordEvent,
}

impl SessionRecordRow {
    pub fn write_to(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = bincode::serialized_size(self).expect("Should calc bincode_size") as usize;
        if len > buf.len() {
            Err(std::io::Error::new(std::io::ErrorKind::OutOfMemory, "Buffer too small"))
        } else {
            bincode::serialize_into(buf, self).expect("Should serialize ok");
            Ok(len)
        }
    }

    pub fn read_from(buf: &[u8]) -> std::io::Result<Self> {
        bincode::deserialize(buf).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bincode deserialize error"))
    }
}
