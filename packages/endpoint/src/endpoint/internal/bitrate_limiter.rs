use cluster::BitrateControlMode;

const DEFAULT_CONSUMER_LIMIT: u32 = 100000;
const IDLE_BITRATE_RECV_LIMIT: u32 = 100_000; //100kbps

pub struct BitrateLimiter {
    typ: BitrateControlMode,
    max_bitrate: u32,
    sum_bitrate: u32,
}

impl BitrateLimiter {
    pub fn new(typ: BitrateControlMode, max_bitrate: u32) -> Self {
        Self { typ, max_bitrate, sum_bitrate: 0 }
    }

    pub fn reset(&mut self) {
        self.sum_bitrate = 0;
    }

    pub fn add_remote(&mut self, consumer_limit: Option<u32>) {
        self.sum_bitrate += consumer_limit.unwrap_or(DEFAULT_CONSUMER_LIMIT);
    }

    pub fn final_bitrate(&self) -> u32 {
        match self.typ {
            BitrateControlMode::MaxBitrateOnly => self.max_bitrate,
            BitrateControlMode::DynamicWithConsumers => self.max_bitrate.min(self.sum_bitrate).max(IDLE_BITRATE_RECV_LIMIT),
        }
    }
}
