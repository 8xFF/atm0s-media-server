use std::time::{SystemTime, UNIX_EPOCH};
pub fn now_ms() -> u64 {
    let start = SystemTime::now();
    start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis() as u64
}
