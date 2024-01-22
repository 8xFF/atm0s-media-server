use media_utils::SeqRewrite;
use transport::PayloadCodec;

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
                    // in case we dont send the packet, client will detect packet loss and request keyframe
                    if is_old_pic_id || (svc.temporal <= self.temporal && (*is_key || !self.key_only)) {
                        log::debug!("[Vp9Selection] select {} {}", is_old_pic_id, svc.temporal);
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
                    if let Some(pic_id) = svc.picture_id {
                        pic_id_rewrite.drop_value(pic_id as u64);
                    }
                    FilterResult::Drop
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
                    //uplayer, because svc only has key-frame in base-layer
                    svc.spatial <= self.spatial && svc.temporal <= self.temporal && *is_key
                } else if current.spatial > self.spatial {
                    //downlayer
                    // In K-SVC we must wait for a keyframe.
                    if self.k_svc {
                        svc.spatial <= self.spatial && svc.temporal <= self.temporal && *is_key
                    } else {
                        // In full SVC we do not need a keyframe.
                        svc.end_frame
                    }
                } else {
                    if self.temporal > current.temporal {
                        svc.temporal <= self.temporal && svc.switching_point
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

impl Vp9SvcFilter {
    pub fn new(k_svc: bool) -> Self {
        Self {
            k_svc,
            spatial_layers: 1,
            ..Default::default()
        }
    }
}

impl ScalableFilter for Vp9SvcFilter {
    fn pause(&mut self) {
        self.current = None;
        self.target = None;
    }

    fn resume(&mut self) {}

    fn set_target_layer(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool {
        let (key_frame, changed) = match &self.current {
            Some(current) => {
                if self.k_svc {
                    (current.spatial != spatial, current.spatial != spatial || current.temporal != temporal)
                } else {
                    (current.spatial < spatial, current.spatial != spatial || current.temporal != temporal)
                }
            }
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
                    log::info!("[Vp9SvcFilter] spatial_layers: {}", spatial_layers);
                    self.spatial_layers = spatial_layers;
                }
            }
            _ => {}
        }

        if let Some(target) = &self.target {
            if target.should_switch(&self.current, pkt) {
                log::info!("[Vp9SvcFilter] switch to spatial: {}, temporal: {}", target.spatial, target.temporal);
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
    use transport::{MediaPacket, PayloadCodec, Vp9Svc};

    use crate::endpoint::internal::local_track::scalable_filter::{FilterResult, ScalableFilter};

    enum Input {
        // input (spatial, temporal, key_only) => need out request key
        SetTarget(u8, u8, bool, bool),
        // input (is_key, spatial, temporal, endframe, switching_point, spatial_layers, pic_id, seq, time) => (FilterResult, switched, pic_id, tl01)
        Packet(bool, u8, u8, bool, bool, Option<u8>, Option<u16>, u16, u32, (FilterResult, Option<u16>)),
    }

    fn test(k_svc: bool, data: Vec<Input>) {
        let mut filter = super::Vp9SvcFilter::new(k_svc);

        let mut index = 0;
        for row in data {
            index += 1;
            match row {
                Input::SetTarget(spatial, temporal, key_only, need_key) => {
                    assert_eq!(filter.set_target_layer(spatial, temporal, key_only), need_key, "index: {}", index);
                }
                Input::Packet(is_key, spatial, temporal, end_frame, switching_point, spatial_layers, pic_id, seq, time, (result, exp_pic_id)) => {
                    let mut pkt = MediaPacket::simple_video(
                        PayloadCodec::Vp9(
                            is_key,
                            transport::Vp9Profile::P0,
                            Some(Vp9Svc {
                                spatial,
                                temporal,
                                begin_frame: false,
                                end_frame,
                                switching_point,
                                picture_id: pic_id,
                                predicted_frame: false,
                                spatial_layers,
                            }),
                        ),
                        seq,
                        time,
                        vec![1, 2, 3],
                    );
                    let res = filter.should_send(&mut pkt);
                    assert_eq!(res, (result, false), "index: {}", index);
                    if matches!(res.0, FilterResult::Send) {
                        match &pkt.codec {
                            PayloadCodec::Vp9(_, _, Some(svc)) => {
                                assert_eq!(svc.picture_id, exp_pic_id, "index: {}", index);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    //TODO refactor tests

    #[test]
    fn simple_non_ksvc_up_down_spatial() {
        test(
            false,
            vec![
                Input::SetTarget(0, 1, false, true),
                Input::Packet(false, 0, 0, false, false, None, Some(1), 0, 100, (FilterResult::Reject, None)),
                Input::Packet(true, 0, 0, false, true, Some(2), Some(2), 1, 200, (FilterResult::Send, Some(2))),
                Input::Packet(true, 0, 2, false, false, None, Some(3), 2, 300, (FilterResult::Drop, None)),
                //Up layer => need keyframe
                Input::SetTarget(1, 2, false, true),
                Input::Packet(false, 0, 0, false, false, None, Some(4), 3, 400, (FilterResult::Send, Some(3))),
                Input::Packet(false, 1, 0, true, true, None, Some(5), 4, 500, (FilterResult::Drop, None)),
                //Found keyframe => switch to target 1
                Input::Packet(true, 1, 0, false, false, None, Some(6), 5, 600, (FilterResult::Send, Some(4))),
                Input::Packet(false, 0, 0, false, false, None, Some(7), 6, 700, (FilterResult::Send, Some(5))),
                Input::Packet(true, 1, 2, false, false, None, Some(8), 7, 800, (FilterResult::Send, Some(6))),
                //Down layer => wait end frame only
                Input::SetTarget(0, 2, false, false),
                Input::Packet(false, 1, 0, true, false, None, Some(9), 8, 900, (FilterResult::Drop, None)),
                Input::Packet(false, 0, 0, false, false, None, Some(10), 9, 1000, (FilterResult::Send, Some(7))),
            ],
        )
    }

    #[test]
    fn simple_non_ksvc_up_down_temporal() {
        test(
            false,
            vec![
                Input::SetTarget(0, 1, false, true),
                Input::Packet(false, 0, 0, false, false, None, None, 0, 100, (FilterResult::Reject, None)),
                Input::Packet(true, 0, 0, false, true, Some(2), None, 1, 200, (FilterResult::Send, None)),
                Input::Packet(true, 0, 2, false, false, None, None, 2, 300, (FilterResult::Drop, None)),
                //Up temporal => need switching point
                Input::SetTarget(0, 2, false, false),
                Input::Packet(false, 0, 0, false, false, None, None, 3, 400, (FilterResult::Send, None)),
                Input::Packet(false, 0, 2, false, false, None, None, 4, 500, (FilterResult::Drop, None)),
                //Found switching point => switch to target 2
                Input::Packet(false, 0, 0, false, true, None, None, 5, 600, (FilterResult::Send, None)),
                Input::Packet(false, 0, 2, false, false, None, None, 6, 700, (FilterResult::Send, None)),
                Input::Packet(true, 0, 2, false, false, None, None, 7, 800, (FilterResult::Send, None)),
                //Down temporal => wait end frame only
                Input::SetTarget(0, 1, false, false),
                Input::Packet(false, 0, 0, true, false, None, None, 8, 900, (FilterResult::Send, None)),
                Input::Packet(false, 0, 2, false, false, None, None, 9, 1000, (FilterResult::Drop, None)),
            ],
        )
    }

    #[test]
    fn simple_ksvc_up_down_spatial() {
        test(
            true,
            vec![
                Input::SetTarget(0, 1, false, true),
                Input::Packet(false, 0, 0, false, false, None, None, 0, 100, (FilterResult::Reject, None)),
                Input::Packet(true, 0, 0, false, true, Some(2), None, 1, 200, (FilterResult::Send, None)),
                Input::Packet(true, 0, 2, false, false, None, None, 2, 300, (FilterResult::Drop, None)),
                //Up layer => need keyframe
                Input::SetTarget(1, 2, false, true),
                Input::Packet(false, 0, 0, false, false, None, None, 3, 400, (FilterResult::Send, None)),
                Input::Packet(false, 1, 0, false, false, None, None, 4, 500, (FilterResult::Drop, None)),
                //Found keyframe => switch to target 1
                Input::Packet(true, 1, 0, false, false, None, None, 5, 600, (FilterResult::Send, None)),
                Input::Packet(false, 0, 0, false, false, None, None, 6, 700, (FilterResult::Send, None)),
                Input::Packet(true, 1, 2, false, false, None, None, 7, 800, (FilterResult::Send, None)),
                //Down layer => need keyframe
                Input::SetTarget(0, 2, false, true),
                Input::Packet(true, 0, 0, false, false, None, None, 8, 1000, (FilterResult::Send, None)),
                Input::Packet(true, 1, 0, false, false, None, None, 9, 900, (FilterResult::Drop, None)),
            ],
        )
    }

    #[test]
    fn simple_ksvc_up_down_temporal() {
        test(
            false,
            vec![
                Input::SetTarget(0, 1, false, true),
                Input::Packet(false, 0, 0, false, false, None, None, 0, 100, (FilterResult::Reject, None)),
                Input::Packet(true, 0, 0, false, true, Some(2), None, 1, 200, (FilterResult::Send, None)),
                Input::Packet(true, 0, 2, false, false, None, None, 2, 300, (FilterResult::Drop, None)),
                //Up temporal => need switching point
                Input::SetTarget(0, 2, false, false),
                Input::Packet(false, 0, 0, false, false, None, None, 3, 400, (FilterResult::Send, None)),
                Input::Packet(false, 0, 2, false, false, None, None, 4, 500, (FilterResult::Drop, None)),
                //Found switching point => switch to target 2
                Input::Packet(false, 0, 0, false, true, None, None, 5, 600, (FilterResult::Send, None)),
                Input::Packet(false, 0, 2, false, false, None, None, 6, 700, (FilterResult::Send, None)),
                Input::Packet(true, 0, 2, false, false, None, None, 7, 800, (FilterResult::Send, None)),
                //Down temporal => wait end frame only
                Input::SetTarget(0, 1, false, false),
                Input::Packet(false, 0, 0, true, false, None, None, 8, 900, (FilterResult::Send, None)),
                Input::Packet(false, 0, 2, false, false, None, None, 9, 1000, (FilterResult::Drop, None)),
            ],
        )
    }
}
