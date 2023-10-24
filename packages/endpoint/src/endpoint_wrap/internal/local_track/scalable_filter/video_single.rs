use super::{FilterResult, ScalableFilter};

pub struct VideoSingleFilter {
    wait_key: bool,
}

impl Default for VideoSingleFilter {
    fn default() -> Self {
        Self { wait_key: true }
    }
}

impl ScalableFilter for VideoSingleFilter {
    fn pause(&mut self) {
        self.wait_key = true;
    }

    fn resume(&mut self) {}

    fn set_target_layer(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool {
        false
    }

    fn should_send(&mut self, pkt: &mut transport::MediaPacket) -> FilterResult {
        if self.wait_key {
            if pkt.codec.is_key() {
                self.wait_key = false;
            } else {
                return FilterResult::Reject;
            }
        }
        FilterResult::Send
    }
}
