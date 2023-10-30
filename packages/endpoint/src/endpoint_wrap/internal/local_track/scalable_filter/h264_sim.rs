use transport::PayloadCodec;

use super::{FilterResult, ScalableFilter};

struct Selection {
    spatial: u8,
    key_only: bool,
}

impl Selection {
    pub fn new(spatial: u8, key_only: bool) -> Self {
        Self { spatial, key_only }
    }

    pub fn allow(&self, pkt: &transport::MediaPacket) -> FilterResult {
        match &pkt.codec {
            PayloadCodec::H264(is_key, _, Some(sim)) => {
                if sim.spatial == self.spatial && (*is_key || !self.key_only) {
                    FilterResult::Send
                } else {
                    FilterResult::Reject
                }
            }
            _ => FilterResult::Reject,
        }
    }

    pub fn should_switch(&self, pkt: &transport::MediaPacket) -> bool {
        match &pkt.codec {
            PayloadCodec::H264(is_key, _, Some(sim)) => sim.spatial == self.spatial && *is_key,
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct H264SimulcastFilter {
    current: Option<Selection>,
    target: Option<Selection>,
}

impl ScalableFilter for H264SimulcastFilter {
    fn pause(&mut self) {
        self.current = None;
        self.target = None;
    }

    fn resume(&mut self) {}

    fn set_target_layer(&mut self, spatial: u8, _temporal: u8, key_only: bool) -> bool {
        match &self.current {
            Some(current) => {
                if current.spatial != spatial {
                    self.target = Some(Selection::new(spatial, key_only));
                    true
                } else {
                    false
                }
            }
            None => {
                self.target = Some(Selection::new(spatial, key_only));
                true
            }
        }
    }

    fn should_send(&mut self, pkt: &mut transport::MediaPacket) -> (FilterResult, bool) {
        let mut stream_changed = false;
        if let Some(target) = &self.target {
            if target.should_switch(pkt) {
                stream_changed = true;

                log::info!("[H264SimulcastFilter] switch to spatial: {}", target.spatial);
                self.current = self.target.take();
            }
        }

        if let Some(current) = &self.current {
            (current.allow(pkt), stream_changed)
        } else {
            (FilterResult::Reject, stream_changed)
        }
    }
}

#[cfg(test)]
mod test {
    use transport::{H264Profile, H264Simulcast, MediaPacket, PayloadCodec};

    use crate::endpoint_wrap::internal::local_track::scalable_filter::{FilterResult, ScalableFilter};

    enum Input {
        // input (spatial, key_only) => need out request key
        SetTarget(u8, bool, bool),
        // input (is_key, spatial, seq, time) => should send
        Packet(bool, u8, u16, u32, (FilterResult, bool)),
    }

    fn test(data: Vec<Input>) {
        let mut filter = super::H264SimulcastFilter::default();

        for row in data {
            match row {
                Input::SetTarget(spatial, key_only, need_key) => {
                    assert_eq!(filter.set_target_layer(spatial, 2, key_only), need_key);
                }
                Input::Packet(is_key, spatial, seq, time, should_send) => {
                    let mut pkt = MediaPacket::simple_video(
                        PayloadCodec::H264(is_key, H264Profile::P42001fNonInterleaved, Some(H264Simulcast::new(spatial))),
                        seq,
                        time,
                        vec![1, 2, 3],
                    );
                    assert_eq!(filter.should_send(&mut pkt), should_send);
                }
            }
        }
    }

    #[test]
    fn should_active_after_key() {
        test(vec![
            Input::SetTarget(1, false, true),
            // wait key for active target 1
            Input::Packet(false, 1, 1, 1000, (FilterResult::Reject, false)),
            Input::Packet(true, 1, 2, 1000, (FilterResult::Send, true)),
            Input::Packet(false, 1, 3, 1000, (FilterResult::Send, false)),
            Input::Packet(false, 0, 1, 1000, (FilterResult::Reject, false)),
            // up layer
            Input::SetTarget(2, false, true),
            // wait key for active target 2
            Input::Packet(false, 2, 1, 1000, (FilterResult::Reject, false)),
            Input::Packet(true, 2, 2, 1000, (FilterResult::Send, true)),
            Input::Packet(false, 2, 3, 1000, (FilterResult::Send, false)),
            Input::Packet(false, 1, 4, 1000, (FilterResult::Reject, false)),
            // down layer
            Input::SetTarget(0, false, true),
            // wait key for active target 0
            Input::Packet(false, 0, 2, 1000, (FilterResult::Reject, false)),
            Input::Packet(true, 0, 3, 1000, (FilterResult::Send, true)),
            Input::Packet(false, 0, 4, 1000, (FilterResult::Send, false)),
            Input::Packet(false, 1, 5, 1000, (FilterResult::Reject, false)),
        ]);
    }
}
