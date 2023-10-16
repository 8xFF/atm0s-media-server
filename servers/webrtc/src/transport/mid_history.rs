use std::collections::HashMap;

#[derive(Default)]
pub struct MidHistory {
    history: HashMap<u32, u16>,
}

impl MidHistory {
    pub fn get(&mut self, mid: Option<u16>, ssrc: u32) -> Option<u16> {
        if let Some(mid) = mid {
            if !self.history.contains_key(&ssrc) {
                self.history.insert(ssrc, mid);
            }
            Some(mid)
        } else {
            self.history.get(&ssrc).copied()
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn should_work() {
        let mut history = super::MidHistory::default();
        assert_eq!(history.get(Some(1), 1), Some(1));
        assert_eq!(history.get(None, 1), Some(1));
        assert_eq!(history.get(None, 2), None);
        assert_eq!(history.get(Some(2), 2), Some(2));
        assert_eq!(history.get(None, 2), Some(2));
        assert_eq!(history.get(None, 1), Some(1));
    }
}
