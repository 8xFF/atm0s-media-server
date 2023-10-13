use str0m::media::{MediaKind, Mid};
use transport::TrackId;

//TODO optimize this
pub fn mid_to_track(mid: &Mid) -> TrackId {
    let mut track = 0;
    for c in mid.as_bytes() {
        track *= 10;
        track += *c as u16;
    }
    track
}

//TODO optimize this
pub fn track_to_mid(track_id: TrackId) -> Mid {
    Mid::from_array([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, (track_id / 100) as u8, ((track_id % 100) / 10) as u8, (track_id % 10) as u8])
}

pub fn to_transport_kind(value: MediaKind) -> transport::MediaKind {
    match value {
        MediaKind::Audio => transport::MediaKind::Audio,
        MediaKind::Video => transport::MediaKind::Video,
    }
}

#[cfg(test)]
mod tests {
    fn test_mid() {
        let mid = "100".into();
        let track = super::mid_to_track(&mid);
        assert_eq!(track, 100);
        let mid = super::track_to_mid(track);
        assert_eq!(mid, "100".into());
    }
}
