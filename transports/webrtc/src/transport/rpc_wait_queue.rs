use std::collections::HashMap;

pub struct RpcWaitQueue<E> {
    tracks: HashMap<String, Vec<E>>,
}

impl<E> Default for RpcWaitQueue<E> {
    fn default() -> Self {
        Self { tracks: HashMap::new() }
    }
}

impl<E> RpcWaitQueue<E> {
    pub fn put(&mut self, track: &str, e: E) {
        if let Some(track) = self.tracks.get_mut(track) {
            track.push(e);
        } else {
            self.tracks.insert(track.to_string(), vec![e]);
        }
    }

    pub fn take(&mut self, track: &str) -> Vec<E> {
        self.tracks.remove(track).unwrap_or(vec![])
    }
}
