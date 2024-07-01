use crate::{
    endpoint::{PeerId, RoomId, TrackMeta, TrackName},
    media::MediaPacket,
    transport::RemoteTrackId,
};

#[derive(Debug, PartialEq, Clone)]
pub enum SessionRecordEvent {
    JoinRoom(RoomId, PeerId),
    LeaveRoom,
    TrackStarted(RemoteTrackId, TrackName, TrackMeta),
    TrackStopped(RemoteTrackId),
    TrackMedia(RemoteTrackId, MediaPacket),
}
