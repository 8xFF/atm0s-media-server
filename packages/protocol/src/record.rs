use std::io::Read;

use serde::{Deserialize, Serialize};

use crate::{
    endpoint::{PeerId, RoomId, TrackMeta, TrackName},
    media::MediaPacket,
    transport::RemoteTrackId,
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum SessionRecordEvent {
    JoinRoom(RoomId, PeerId),
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

impl Read for SessionRecordRow {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = bincode::serialized_size(self).expect("Should calc bincode_size") as usize;
        if len > buf.len() {
            Err(std::io::Error::new(std::io::ErrorKind::OutOfMemory, "Buffer too small"))
        } else {
            bincode::serialize_into(buf, self).expect("Should serialize ok");
            Ok(len)
        }
    }
}
