use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Timer: Send + Sync {
    fn now_ms(&self) -> u64;
}

#[derive(Clone)]
pub struct SystemTimer();

impl Timer for SystemTimer {
    fn now_ms(&self) -> u64 {
        let start = SystemTime::now();
        start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis() as u64
    }
}

#[derive(Clone, Default)]
pub struct MockTimer {
    current_value: Arc<AtomicU64>,
}

impl Timer for MockTimer {
    fn now_ms(&self) -> u64 {
        self.current_value.load(Ordering::Relaxed)
    }
}

impl MockTimer {
    pub fn fake(&self, value: u64) {
        self.current_value.store(value, Ordering::Relaxed);
    }
}
