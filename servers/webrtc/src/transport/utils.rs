use str0m::media::Mid;
use transport::TrackId;

pub fn mid_to_track(mid: &Mid) -> TrackId {
    let mut track = 0;
    for c in mid.as_bytes() {
        track *= 10;
        track += *c as u16;
    }
    track
}

//TODO test this
