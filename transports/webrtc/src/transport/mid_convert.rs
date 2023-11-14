use str0m::media::{MediaKind, Mid, Rid};
use transport::TrackId;

const ZERO_CHAR: u8 = 48;

//TODO optimize this
pub fn mid_to_track(mid: &Mid) -> TrackId {
    let mut track = 0;
    for c in mid.as_bytes() {
        track *= 10;
        track += (*c - ZERO_CHAR) as u16;
    }
    track
}

pub fn rid_to_u16(rid: &Rid) -> TrackId {
    let mut value = 0;
    for c in rid.as_bytes() {
        value *= 10;
        value += (*c - ZERO_CHAR) as u16;
    }
    value
}

//TODO optimize this
pub fn track_to_mid(track_id: TrackId) -> Mid {
    if track_id < 10 {
        Mid::from_array([track_id as u8 + ZERO_CHAR, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32])
    } else if track_id < 100 {
        Mid::from_array([
            (track_id / 10) as u8 + ZERO_CHAR,
            (track_id % 10) as u8 + ZERO_CHAR,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
        ])
    } else if track_id < 1000 {
        Mid::from_array([
            (track_id / 100) as u8 + ZERO_CHAR,
            ((track_id % 100) / 10) as u8 + ZERO_CHAR,
            (track_id % 10) as u8 + ZERO_CHAR,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
            32,
        ])
    } else {
        panic!("not supported");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mid_to_track() {
        let mid = Mid::from("100");
        let track = mid_to_track(&mid);
        assert_eq!(track, 100);
    }

    #[test]
    fn test_rid_to_u16() {
        let rid = Rid::from("100");
        let value = rid_to_u16(&rid);
        assert_eq!(value, 100);
    }

    #[test]
    fn test_track_to_mid() {
        let track_id = 100;
        let mid = track_to_mid(track_id);
        assert_eq!(mid, Mid::from("100"));
    }
}
