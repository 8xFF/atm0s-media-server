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
