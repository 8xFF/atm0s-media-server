use transport::PayloadCodec;
use utils::SeqRewrite;

use super::{FilterResult, ScalableFilter};

const PIC_ID_MAX: u64 = 1 << 15;

struct Selection {
    k_svc: bool,
    spatial: u8,
    temporal: u8,
    key_only: bool,
}

impl Selection {
    pub fn new(k_svc: bool, spatial: u8, temporal: u8, key_only: bool) -> Self {
        Self { k_svc, spatial, temporal, key_only }
    }

    pub fn allow(&self, spatial_layers: u8, pkt: &mut transport::MediaPacket, pic_id_rewrite: &mut SeqRewrite<PIC_ID_MAX, 60>) -> FilterResult {
        match &mut pkt.codec {
            PayloadCodec::Vp9(is_key, _, Some(svc)) => {
                if svc.spatial <= self.spatial {
                    let is_old_pic_id = if let Some(pic_id) = svc.picture_id {
                        pic_id_rewrite.is_seq_lower_than(pic_id as u64, pic_id_rewrite.max_input())
                    } else {
                        false
                    };

                    // in case of old pic_id that mean have some unordered packets, so we should sending this
                    // if not, client will detect packet loss and request keyframe
                    if is_old_pic_id || (svc.temporal <= self.temporal && (*is_key || !self.key_only)) {
                        if let Some(pic_id) = svc.picture_id {
                            if let Some(new_pic_id) = pic_id_rewrite.generate(pic_id as u64) {
                                svc.picture_id = Some(new_pic_id as u16);
                            } else {
                                return FilterResult::Drop;
                            }
                        }
                        if (svc.spatial == self.spatial || svc.spatial + 1 == spatial_layers) && svc.end_frame {
                            pkt.marker = true;
                        }
                        FilterResult::Send
                    } else {
                        if let Some(pic_id) = svc.picture_id {
                            pic_id_rewrite.drop_value(pic_id as u64);
                        }
                        FilterResult::Drop
                    }
                } else {
                    FilterResult::Reject
                }
            }
            _ => FilterResult::Reject,
        }
    }

    pub fn should_switch(&self, current: &Option<Self>, pkt: &transport::MediaPacket) -> bool {
        match (current, &pkt.codec) {
            (None, PayloadCodec::Vp9(is_key, _, Some(svc))) => svc.spatial == self.spatial && svc.temporal <= self.temporal && *is_key,
            (Some(current), PayloadCodec::Vp9(is_key, _, Some(svc))) => {
                if current.spatial < self.spatial {
                    //uplayer
                    svc.spatial == self.spatial && svc.temporal <= self.temporal && *is_key
                } else if current.spatial > self.spatial {
                    //downlayer
                    // In K-SVC we must wait for a keyframe.
                    if self.k_svc {
                        *is_key
                    } else {
                        // In full SVC we do not need a keyframe.
                        svc.end_frame
                    }
                } else {
                    if self.temporal > current.temporal {
                        svc.switching_point
                    } else {
                        svc.end_frame
                    }
                }
            }
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct Vp9SvcFilter {
    k_svc: bool,
    current: Option<Selection>,
    target: Option<Selection>,
    pic_id_rewrite: SeqRewrite<PIC_ID_MAX, 60>,
    spatial_layers: u8,
}

impl ScalableFilter for Vp9SvcFilter {
    fn pause(&mut self) {
        self.current = None;
        self.target = None;
    }

    fn resume(&mut self) {}

    fn set_target_layer(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool {
        let (key_frame, changed) = match &self.current {
            Some(current) => (current.spatial != spatial, current.spatial != spatial || current.temporal != temporal),
            None => (true, true),
        };
        if changed {
            self.target = Some(Selection::new(self.k_svc, spatial, temporal, key_only));
        }
        key_frame
    }

    fn should_send(&mut self, pkt: &mut transport::MediaPacket) -> (FilterResult, bool) {
        match &pkt.codec {
            PayloadCodec::Vp9(_, _, Some(svc)) => {
                if let Some(spatial_layers) = svc.spatial_layers {
                    self.spatial_layers = spatial_layers;
                }
            }
            _ => {}
        }

        if let Some(target) = &self.target {
            if target.should_switch(&self.current, pkt) {
                self.current = self.target.take();
            }
        }

        if let Some(current) = &self.current {
            (current.allow(self.spatial_layers, pkt, &mut self.pic_id_rewrite), false)
        } else {
            (FilterResult::Reject, false)
        }
    }
}

#[cfg(test)]
mod test {
    // use transport::{MediaPacket, PayloadCodec, Vp9Profile, Vp9Svc};

    // use crate::endpoint_wrap::internal::local_track::scalable_filter::{ScalableFilter, FilterResult};

    // enum Input {
    //     // input (spatial, temporal, key_only) => need out request key
    //     SetTarget(u8, u8, bool, bool),
    //     // input (is_key, spatial, temporal, layer_sync, seq, time) => should send
    //     Packet(bool, u8, u8, bool, u16, u32, FilterResult),
    // }

    // fn test(data: Vec<Input>) {
    //     let mut filter = super::Vp9SvcFilter::default();

    //     for row in data {
    //         match row {
    //             Input::SetTarget(spatial, temporal, key_only, need_key) => {
    //                 assert_eq!(filter.set_target_layer(spatial, temporal, key_only), need_key);
    //             }
    //             Input::Packet(is_key, spatial, temporal, layer_sync, seq, time, send_expected) => {
    //                 let mut pkt = MediaPacket::simple_video(PayloadCodec::Vp9(is_key, Vp9Profile::P0, Some(Vp9Svc::new(spatial, temporal, layer_sync))), seq, time, vec![1, 2, 3]);
    //                 assert_eq!(filter.should_send(&mut pkt), send_expected);
    //             }
    //         }
    //     }
    // }

    // #[test]
    // fn simple() {
    //     test(vec![
    //         Input::SetTarget(0, 1, false, true),
    //         Input::Packet(false, 0, 0, false, 0, 100, FilterResult::Reject),
    //         Input::Packet(true, 0, 0, true, 1, 200, FilterResult::Send),
    //         Input::Packet(true, 0, 2, true, 2, 200, FilterResult::Drop),
    //     ])
    // }
}
