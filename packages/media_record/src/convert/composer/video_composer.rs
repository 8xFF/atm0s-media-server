use media_server_protocol::{
    endpoint::{TrackMeta, TrackName},
    media::MediaPacket,
    transport::RemoteTrackId,
};

#[derive(Default)]
pub struct VideoComposer {}

impl VideoComposer {
    pub fn add_track(&mut self, session_id: u64, remote_track_id: RemoteTrackId, track_name: TrackName, track_meta: TrackMeta) {
        log::info!("add track {:?} {:?} {:?} {:?}", session_id, remote_track_id, track_name, track_meta);
    }

    pub fn on_media(&mut self, session_id: u64, remote_track_id: RemoteTrackId, media_packet: MediaPacket) {
        log::info!("on media {:?} {:?} {:?}", session_id, remote_track_id, media_packet.seq);
    }

    pub fn remove_track(&mut self, session_id: u64, remote_track_id: RemoteTrackId) {
        log::info!("remove track {:?} {:?}", session_id, remote_track_id);
    }
}
