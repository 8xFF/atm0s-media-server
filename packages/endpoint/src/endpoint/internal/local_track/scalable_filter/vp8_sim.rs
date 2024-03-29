//! VP8 simulcast filter
//! - Rewrite VP8 PictureId and TTL0IDX.
//!
//! For PictureId: it will rewrite the PictureId to avoid PictureId conflict.
//! For TL0IDX:
//!
//! Ref: https://github.com/versatica/mediasoup/issues/40
//!
//! TL0PICIDX:  8 bits temporal level zero index.  TL0PICIDX is a
//! running index for the temporal base layer frames, i.e., the
//! frames with TID set to 0.  If TID is larger than 0, TL0PICIDX
//! indicates on which base-layer frame the current image depends.
//! TL0PICIDX MUST be incremented when TID is 0.  The index MAY
//! start at a random value, and it MUST wrap to 0 after reaching
//! the maximum number 255.  Use of TL0PICIDX depends on the
//! presence of TID.  Therefore, it is RECOMMENDED that the TID be
//! used whenever TL0PICIDX is.

use media_utils::SeqRewrite;
use transport::PayloadCodec;

use super::{FilterResult, ScalableFilter};

const PIC_ID_MAX: u64 = 1 << 15;
const TL0IDX_MAX: u64 = 1 << 8;

struct Selection {
    spatial: u8,
    temporal: u8,
    key_only: bool,
}

impl Selection {
    pub fn new(spatial: u8, temporal: u8, key_only: bool) -> Self {
        Self { spatial, temporal, key_only }
    }

    pub fn allow(&self, pkt: &mut transport::MediaPacket, pic_id_rewrite: &mut SeqRewrite<PIC_ID_MAX, 60>, tl0idx_rewrite: &mut SeqRewrite<TL0IDX_MAX, 60>) -> FilterResult {
        match &mut pkt.codec {
            PayloadCodec::Vp8(is_key, Some(sim)) => {
                if sim.spatial == self.spatial {
                    if sim.temporal <= self.temporal && (*is_key || !self.key_only) {
                        if let Some(tl0idx) = sim.tl0_pic_idx {
                            if let Some(new_tl0idx) = tl0idx_rewrite.generate(tl0idx as u64) {
                                sim.tl0_pic_idx = Some(new_tl0idx as u8);
                            } else {
                                return FilterResult::Drop;
                            }
                        }
                        if let Some(pic_id) = sim.picture_id {
                            if let Some(new_pic_id) = pic_id_rewrite.generate(pic_id as u64) {
                                sim.picture_id = Some(new_pic_id as u16);
                            } else {
                                return FilterResult::Drop;
                            }
                        }

                        FilterResult::Send
                    } else {
                        if let Some(pic_id) = sim.picture_id {
                            pic_id_rewrite.drop_value(pic_id as u64);
                        }
                        // We dont need drop tl01picidx because it only increment in base layer with TID (temporal) = 0, which never drop
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
            (None, PayloadCodec::Vp8(is_key, Some(sim))) => sim.spatial == self.spatial && sim.temporal <= self.temporal && *is_key,
            (Some(current), PayloadCodec::Vp8(is_key, Some(sim))) => {
                if current.spatial == self.spatial {
                    if self.temporal > current.temporal {
                        //Up sample
                        sim.temporal == self.temporal && sim.layer_sync
                    } else {
                        //Down sample => should apply now
                        true
                    }
                } else {
                    sim.spatial == self.spatial && sim.temporal <= self.temporal && *is_key
                }
            }
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct Vp8SimulcastFilter {
    current: Option<Selection>,
    target: Option<Selection>,
    pic_id_rewrite: SeqRewrite<PIC_ID_MAX, 60>,
    tl0idx_rewrite: SeqRewrite<TL0IDX_MAX, 60>,
}

impl ScalableFilter for Vp8SimulcastFilter {
    fn pause(&mut self) {
        self.current = None;
        self.target = None;
        self.pic_id_rewrite.reinit();
        self.tl0idx_rewrite.reinit();
    }

    fn resume(&mut self) {}

    fn set_target_layer(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool {
        let (key_frame, changed) = match &self.current {
            Some(current) => (current.spatial != spatial, current.spatial != spatial || current.temporal != temporal),
            None => (true, true),
        };
        if changed {
            self.target = Some(Selection::new(spatial, temporal, key_only));
        }
        key_frame
    }

    fn should_send(&mut self, pkt: &mut transport::MediaPacket) -> (FilterResult, bool) {
        let mut stream_changed = false;
        if let Some(target) = &self.target {
            if target.should_switch(&self.current, pkt) {
                stream_changed = if let Some(current) = &self.current {
                    current.spatial != target.spatial
                } else {
                    true
                };

                if stream_changed {
                    self.pic_id_rewrite.reinit();
                    self.tl0idx_rewrite.reinit();
                }

                log::info!("[Vp8SimulcastFilter] switch to spatial: {}, temporal: {}", target.spatial, target.temporal);
                self.current = self.target.take();
            }
        }

        if let Some(current) = &self.current {
            (current.allow(pkt, &mut self.pic_id_rewrite, &mut self.tl0idx_rewrite), stream_changed)
        } else {
            (FilterResult::Reject, stream_changed)
        }
    }
}

#[cfg(test)]
mod test {
    use transport::{MediaPacket, PayloadCodec, Vp8Simulcast};

    use crate::endpoint::internal::local_track::scalable_filter::{FilterResult, ScalableFilter};

    enum Input {
        // input (spatial, temporal, key_only) => need out request key
        SetTarget(u8, u8, bool, bool),
        // input (is_key, spatial, temporal, layer_sync, pic_id, tl01, seq, time) => (FilterResult, switched, pic_id, tl01)
        Packet(bool, u8, u8, Option<u16>, Option<u8>, bool, u16, u32, (FilterResult, bool, Option<u16>, Option<u8>)),
    }

    fn test(data: Vec<Input>) {
        let mut filter = super::Vp8SimulcastFilter::default();

        let mut index = 0;
        for row in data {
            index += 1;
            match row {
                Input::SetTarget(spatial, temporal, key_only, need_key) => {
                    assert_eq!(filter.set_target_layer(spatial, temporal, key_only), need_key, "index: {}", index);
                }
                Input::Packet(is_key, spatial, temporal, pic_id, tl01, layer_sync, seq, time, (result, switched, exp_pic_id, exp_tl01)) => {
                    let mut pkt = MediaPacket::simple_video(
                        PayloadCodec::Vp8(
                            is_key,
                            Some(Vp8Simulcast {
                                spatial,
                                temporal,
                                layer_sync,
                                picture_id: pic_id,
                                tl0_pic_idx: tl01,
                            }),
                        ),
                        seq,
                        time,
                        vec![1, 2, 3],
                    );
                    let res = filter.should_send(&mut pkt);
                    assert_eq!(res, (result, switched), "index: {}", index);
                    if matches!(res.0, FilterResult::Send) {
                        match &pkt.codec {
                            PayloadCodec::Vp8(_, Some(sim)) => {
                                assert_eq!(sim.picture_id, exp_pic_id, "index: {}", index);
                                assert_eq!(sim.tl0_pic_idx, exp_tl01, "index: {}", index);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn simple() {
        test(vec![
            Input::SetTarget(0, 1, false, true),
            Input::Packet(false, 0, 0, None, None, false, 0, 100, (FilterResult::Reject, false, None, None)),
            Input::Packet(true, 0, 0, None, None, true, 1, 200, (FilterResult::Send, true, None, None)),
            Input::Packet(true, 0, 2, None, None, true, 2, 200, (FilterResult::Drop, false, None, None)),
            Input::SetTarget(1, 2, false, true),
            Input::Packet(false, 0, 0, None, None, false, 0, 100, (FilterResult::Send, false, None, None)),
            Input::Packet(false, 1, 0, None, None, true, 1, 200, (FilterResult::Reject, false, None, None)),
            Input::Packet(true, 1, 0, None, None, true, 1, 200, (FilterResult::Send, true, None, None)),
            Input::Packet(true, 1, 2, None, None, true, 2, 200, (FilterResult::Send, false, None, None)),
        ])
    }

    #[test]
    fn rewrite_pic_id_tl01_temporal_remain_increasement() {
        test(vec![
            Input::SetTarget(0, 1, false, true),
            Input::Packet(false, 0, 0, Some(1), Some(1), false, 0, 100, (FilterResult::Reject, false, None, None)),
            Input::Packet(true, 0, 0, Some(2), Some(2), true, 1, 200, (FilterResult::Send, true, Some(1), Some(1))),
            Input::Packet(false, 0, 1, Some(3), Some(2), true, 2, 300, (FilterResult::Send, false, Some(2), Some(1))),
            Input::SetTarget(0, 2, false, false),
            // This packet Drop then need to drop pic_id
            Input::Packet(false, 0, 2, Some(4), Some(2), false, 3, 400, (FilterResult::Drop, false, None, None)),
            Input::Packet(false, 0, 2, Some(5), Some(2), true, 4, 500, (FilterResult::Send, false, Some(3), Some(1))),
            Input::Packet(false, 0, 0, Some(6), Some(3), true, 5, 600, (FilterResult::Send, false, Some(4), Some(2))),
            Input::Packet(false, 0, 1, Some(7), Some(3), true, 6, 700, (FilterResult::Send, false, Some(5), Some(2))),
            Input::Packet(false, 0, 2, Some(8), Some(3), true, 7, 800, (FilterResult::Send, false, Some(6), Some(2))),
            Input::Packet(false, 0, 2, Some(8), Some(3), true, 8, 900, (FilterResult::Send, false, Some(6), Some(2))),
        ])
    }

    #[test]
    fn rewrite_pic_id_tl01_spatial_switch_stream_remain_continuous() {
        test(vec![
            Input::SetTarget(0, 1, false, true),
            Input::Packet(false, 0, 0, Some(1), Some(1), false, 0, 100, (FilterResult::Reject, false, None, None)),
            Input::Packet(true, 0, 0, Some(2), Some(2), true, 1, 200, (FilterResult::Send, true, Some(1), Some(1))),
            Input::Packet(false, 0, 1, Some(3), Some(2), true, 2, 300, (FilterResult::Send, false, Some(2), Some(1))),
            Input::SetTarget(1, 2, false, true),
            // This packet Drop then need to drop pic_id
            Input::Packet(false, 1, 2, Some(98), Some(50), false, 3, 400, (FilterResult::Reject, false, None, None)),
            Input::Packet(false, 1, 2, Some(99), Some(50), true, 4, 500, (FilterResult::Reject, false, None, None)),
            Input::Packet(true, 1, 0, Some(100), Some(51), true, 5, 600, (FilterResult::Send, true, Some(3), Some(2))),
            Input::Packet(false, 1, 1, Some(101), Some(51), true, 6, 700, (FilterResult::Send, false, Some(4), Some(2))),
            Input::Packet(false, 1, 2, Some(102), Some(51), true, 7, 800, (FilterResult::Send, false, Some(5), Some(2))),
            Input::Packet(false, 1, 2, Some(102), Some(51), true, 8, 900, (FilterResult::Send, false, Some(5), Some(2))),
        ])
    }
}
