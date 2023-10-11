use transport::RtpPacket;

pub type ClusterTrackId = u64;
pub type PeerId = String;
pub type TrackId = String;
pub struct TrackMeta {

}

pub enum ClusterRoomError {
    NotImplement
}

pub enum ClusterRoomIncomingEvent {
    Media(ClusterTrackId, RtpPacket),
    StreamAdded(PeerId, TrackId, TrackMeta),
    StreamRemoved(PeerId, TrackId, TrackMeta)
}

pub enum ClusterRoomOutgoingEvent {
    Media(ClusterTrackId, RtpPacket),
    StreamAdded(PeerId, TrackId, TrackMeta),
    StreamRemoved(PeerId, TrackId, TrackMeta)
}

#[async_trait::async_trait]
pub trait ClusterRoom {
    fn on_event(&mut self, event: ClusterRoomOutgoingEvent) -> Result<(), ClusterRoomError>;
    async fn recv(&mut self) -> Result<ClusterRoomIncomingEvent, ClusterRoomError>;
}