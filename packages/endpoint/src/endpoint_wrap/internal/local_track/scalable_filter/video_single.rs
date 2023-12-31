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

    fn set_target_layer(&mut self, _spatial: u8, _temporal: u8, _key_only: bool) -> bool {
        self.wait_key
    }

    fn should_send(&mut self, pkt: &mut transport::MediaPacket) -> (FilterResult, bool) {
        if self.wait_key {
            if pkt.codec.is_key() {
                self.wait_key = false;
            } else {
                return (FilterResult::Reject, false);
            }
        }
        (FilterResult::Send, false)
    }
}
