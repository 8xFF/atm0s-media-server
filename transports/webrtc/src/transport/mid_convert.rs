use str0m::media::{Mid, Rid};
use transport::TrackId;

const ZERO_CHAR: u8 = b'0';

#[derive(Default)]
pub struct MidMapper {
    cache: Vec<Mid>,
}

impl MidMapper {
    /// Returns the mid for the given track id.
    /// If the track id is not known, a new mid is generated and returned.
    /// If the track id is known, the corresponding mid is returned.
    pub fn mid_to_track(&mut self, mid: Mid) -> TrackId {
        let index = self.cache.iter().position(|m| *m == mid);
        match index {
            Some(index) => index as TrackId,
            None => {
                self.cache.push(mid);
                (self.cache.len() - 1) as TrackId
            }
        }
    }

    pub fn track_to_mid(&self, track_id: TrackId) -> Option<&Mid> {
        self.cache.get(track_id as usize)
    }
}

pub fn rid_to_u16(rid: &Rid) -> TrackId {
    let mut value = 0;
    for c in rid.as_bytes() {
        value *= 10;
        value += (*c - ZERO_CHAR) as u16;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mid_to_track() {
        let mut mapper = MidMapper::default();
        assert_eq!(mapper.mid_to_track(Mid::from("100")), 0);
        assert_eq!(mapper.mid_to_track(Mid::from("101")), 1);
        assert_eq!(mapper.mid_to_track(Mid::from("100")), 0);

        //convert back
        assert_eq!(mapper.track_to_mid(0), Some(&Mid::from("100")));
        assert_eq!(mapper.track_to_mid(1), Some(&Mid::from("101")));
        assert_eq!(mapper.track_to_mid(2), None);
    }

    #[test]
    fn test_rid_to_u16() {
        let rid = Rid::from("100");
        let value = rid_to_u16(&rid);
        assert_eq!(value, 100);
    }
}
