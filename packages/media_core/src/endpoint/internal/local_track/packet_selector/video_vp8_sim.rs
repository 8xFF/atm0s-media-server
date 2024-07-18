//! Vp8 simulcast selector
//!
//! This selector take care switch to best layer based on target bitrate and source layers bitrate.
//!
//! In order to switch, it will have own state (pic_id_rewrite and tl0xidx_rewrite) to ensure output stream
//! is not disconnect, and also call drop_value and reinit on shared state (seq_rewrite, ts_rewrite) when
//! it switch to other spatial layer.
//!
//! Note that, in simulcast stream, each spatial layer is independent stream and have independent seq, ts

use std::{cmp::Ordering, collections::VecDeque};

use media_server_protocol::media::{MediaLayerSelection, MediaLayersBitrate, MediaMeta, MediaPacket};
use media_server_utils::SeqRewrite;

use super::{Action, VideoSelector, VideoSelectorCtx};

const PIC_ID_MAX: u64 = 1 << 15;
const TL0IDX_MAX: u64 = 1 << 8;

#[derive(Default)]
pub struct Ctx {
    pic_id_rewrite: SeqRewrite<PIC_ID_MAX, 60>,
    tl0idx_rewrite: SeqRewrite<TL0IDX_MAX, 60>,
}

pub struct Selector {
    bitrate_kbps: u16,
    layers: MediaLayersBitrate,
    current: Option<MediaLayerSelection>,
    target: Option<MediaLayerSelection>,
    queue: VecDeque<Action>,
    limit: (u8, u8),
}

impl Selector {
    pub fn new(bitrate: u64, layers: MediaLayersBitrate, limit: (u8, u8)) -> Self {
        let bitrate_kbps = (bitrate / 1000) as u16;
        let (max_spatial, max_temporal) = limit;
        let target = layers.select_layer(bitrate_kbps, max_spatial, max_temporal);

        log::info!("[Vp8SimSelector] create with bitrate {bitrate_kbps} kbps, layers {:?} => init target {:?}", layers, target);

        Self {
            bitrate_kbps,
            layers,
            current: None,
            target,
            queue: VecDeque::new(),
            limit: (max_spatial, max_temporal),
        }
    }

    fn select_layer(&mut self) {
        let target = self.layers.select_layer(self.bitrate_kbps, self.limit.0, self.limit.1);
        if target != self.target {
            log::info!("[Vp8SimSelector] bitrate {} kbps, layers {:?} => changed target to {:?}", self.bitrate_kbps, self.layers, target);
            self.target = target;

            if let Some(target) = &self.target {
                if self.current.is_none() || target.spatial != self.current.as_ref().expect("Should have").spatial {
                    log::info!("[Vp8SimSelector] switch to new spatial layer => request key frame");
                    self.queue.push_back(Action::RequestKeyFrame);
                }
            }
        }
    }

    fn try_switch(&mut self, ctx: &mut VideoSelectorCtx, pkt: &mut MediaPacket) {
        if self.target == self.current {
            return;
        }
        if let MediaMeta::Vp8 { key, sim: Some(sim) } = &mut pkt.meta {
            match (&mut self.current, &self.target) {
                (Some(current), Some(target)) => {
                    match target.spatial.cmp(&current.spatial) {
                        Ordering::Equal => {
                            // change temporal
                            match target.temporal.cmp(&current.temporal) {
                                Ordering::Greater => {
                                    // up temporal => need wait layer_sync
                                    if sim.spatial == current.spatial && sim.temporal <= target.temporal && sim.layer_sync {
                                        log::info!("[Vp8SimSelector] up temporal {},{} => {},{}", current.spatial, current.temporal, target.spatial, target.temporal);
                                        current.temporal = target.temporal;
                                    }
                                }
                                Ordering::Less => {
                                    // down temporal => do now
                                    log::info!("[Vp8SimSelector] down temporal {},{} => {},{}", current.spatial, current.temporal, target.spatial, target.temporal);
                                    current.temporal = target.temporal;
                                }
                                Ordering::Equal => {}
                            }
                        }
                        Ordering::Less => {
                            // down spatial => need wait key-frame
                            // first we always down temporal for trying to reduce bandwidth
                            if current.temporal != 0 {
                                log::info!("[Vp8SimSelector] down spatial then down temporal from {} => 0", current.temporal);
                                current.temporal = 0;
                            }
                            if *key {
                                log::info!("[Vp8SimSelector] down {},{} => {},{} with key", current.spatial, current.temporal, target.spatial, target.temporal);
                                // with other spatial we have difference tl0xidx and pic_id offset
                                // therefore we need reinit both tl0idx and pic_id
                                ctx.vp8_ctx.tl0idx_rewrite.reinit();
                                ctx.vp8_ctx.pic_id_rewrite.reinit();
                                ctx.seq_rewrite.reinit();
                                ctx.ts_rewrite.reinit();
                                current.spatial = target.spatial;
                                current.temporal = target.temporal;
                            } else {
                                self.queue.push_back(Action::RequestKeyFrame);
                            }
                        }
                        Ordering::Greater => {
                            // up spatial => need wait key-frame
                            // first we try to up temporal for trying to increase bandwidth
                            if sim.spatial == current.spatial && current.temporal != 2 && sim.layer_sync {
                                log::info!("[Vp8SimSelector] up spatial then up temporal from {} => 2 before key arrived", current.temporal);
                                current.temporal = 2;
                            }

                            if *key {
                                log::info!("[Vp8SimSelector] up {},{} => {},{} with key-frame", current.spatial, current.temporal, target.spatial, target.temporal);
                                // with other spatial we have difference tl0xidx and pic_id offset
                                // therefore we need reinit both tl0idx and pic_id
                                ctx.vp8_ctx.tl0idx_rewrite.reinit();
                                ctx.vp8_ctx.pic_id_rewrite.reinit();
                                ctx.seq_rewrite.reinit();
                                ctx.ts_rewrite.reinit();
                                current.spatial = target.spatial;
                                current.temporal = target.temporal;
                            } else {
                                self.queue.push_back(Action::RequestKeyFrame);
                            }
                        }
                    }
                }
                (Some(current), None) => {
                    // need pause
                    // TODO: wait current frame finished for avoiding interrupt client
                    log::info!("[Vp8SimSelector] pause from {},{}", current.spatial, current.temporal);
                    self.current = None;
                }
                (None, Some(target)) => {
                    // need resume or start => need wait key_frame
                    if *key {
                        log::info!("[Vp8SimSelector] resume to {},{} with key", target.spatial, target.temporal);
                        // with other spatial we have difference tl0xidx and pic_id offset
                        // therefore we need reinit both tl0idx and pic_id
                        ctx.vp8_ctx.tl0idx_rewrite.reinit();
                        ctx.vp8_ctx.pic_id_rewrite.reinit();
                        self.current = Some(target.clone());
                    }
                }
                (None, None) => {
                    // reject
                }
            }
        }
    }

    fn is_allow(&mut self, ctx: &mut VideoSelectorCtx, pkt: &mut MediaPacket) -> Option<()> {
        let current = self.current.as_ref()?;
        match &mut pkt.meta {
            MediaMeta::Vp8 { key: _, sim: Some(sim) } => {
                if sim.spatial == current.spatial && sim.temporal <= current.temporal {
                    log::trace!(
                        "[Vp8SimSelector] allow {} {}, seq {}, ts {}, tl0idx {:?} pic_id {:?}",
                        sim.spatial,
                        sim.temporal,
                        pkt.seq,
                        pkt.ts,
                        sim.tl0_pic_idx,
                        sim.picture_id
                    );
                    if let Some(tl0idx) = sim.tl0_pic_idx {
                        sim.tl0_pic_idx = Some(ctx.vp8_ctx.tl0idx_rewrite.generate(tl0idx as u64)? as u8);
                    }

                    if let Some(pic_id) = sim.picture_id {
                        sim.picture_id = Some(ctx.vp8_ctx.pic_id_rewrite.generate(pic_id as u64)? as u16);
                    }

                    Some(())
                } else if sim.spatial == current.spatial {
                    log::trace!("[Vp8SimSelector] reject {} {}, seq {}, ts {}", sim.spatial, sim.temporal, pkt.seq, pkt.ts);
                    // We don't need drop tl01picidx because it only increment in base layer
                    // with TID (temporal) = 0, which never drop
                    if let Some(pic_id) = sim.picture_id {
                        ctx.vp8_ctx.pic_id_rewrite.drop_value(pic_id as u64);
                    }

                    ctx.seq_rewrite.drop_value(pkt.seq as u64);

                    None
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl VideoSelector for Selector {
    fn on_init(&mut self, ctx: &mut VideoSelectorCtx, _now_ms: u64) {
        ctx.vp8_ctx.pic_id_rewrite.reinit();
        ctx.vp8_ctx.tl0idx_rewrite.reinit();
    }

    fn on_tick(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64) {}

    fn set_target_bitrate(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64, bitrate: u64) {
        let bitrate_kbps = (bitrate / 1000) as u16;
        self.bitrate_kbps = bitrate_kbps;
        self.select_layer();
    }

    fn set_limit_layer(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64, max_spatial: u8, max_temporal: u8) {
        self.limit = (max_spatial, max_temporal);
        self.select_layer();
    }

    fn select(&mut self, ctx: &mut VideoSelectorCtx, _now_ms: u64, _channel: u64, pkt: &mut MediaPacket) -> Option<()> {
        if let Some(layers) = pkt.layers.as_ref() {
            self.layers = layers.clone();
            self.select_layer();
        }
        self.try_switch(ctx, pkt);
        self.is_allow(ctx, pkt)
    }

    fn pop_action(&mut self) -> Option<super::Action> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use crate::endpoint::internal::local_track::packet_selector::{VideoSelector, VideoSelectorCtx};

    use super::{Action, Selector};
    use media_server_protocol::media::{MediaKind, MediaLayerBitrate, MediaLayersBitrate, MediaMeta, MediaPacket, Vp8Sim};

    fn layers_bitrate(layers: &[[u16; 3]]) -> MediaLayersBitrate {
        let mut res = MediaLayersBitrate::default();
        for (index, layer) in layers.iter().enumerate() {
            res.set_layer(index, MediaLayerBitrate::new(layer));
        }
        res
    }

    #[allow(clippy::too_many_arguments)]
    fn video_pkt(seq: u16, ts: u32, key: bool, layers: Option<&[[u16; 3]]>, spatial: u8, temporal: u8, layer_sync: bool, tl0idx: u8, pic_id: u16) -> MediaPacket {
        MediaPacket {
            ts,
            seq,
            marker: true,
            nackable: false,
            layers: layers.map(layers_bitrate),
            meta: MediaMeta::Vp8 {
                key,
                sim: Some(Vp8Sim {
                    picture_id: Some(pic_id),
                    tl0_pic_idx: Some(tl0idx),
                    spatial,
                    temporal,
                    layer_sync,
                }),
            },
            data: vec![1, 2, 3],
        }
    }

    enum Step {
        Bitrate(u64, u64, Vec<Action>),
        Pkt(u64, u64, MediaPacket, Option<(u16, u32, u8, u16)>, Vec<Action>),
    }

    fn test(bitrate: u64, layers: &[[u16; 3]], steps: Vec<Step>) {
        let mut ctx = VideoSelectorCtx::new(MediaKind::Video);
        ctx.seq_rewrite.reinit();
        let mut selector = Selector::new(bitrate * 1000, layers_bitrate(layers), (2, 2));
        selector.on_init(&mut ctx, 0);

        for step in steps {
            let actions = match step {
                Step::Bitrate(ts, bitrate, actions) => {
                    selector.set_target_bitrate(&mut ctx, ts, bitrate * 1000);
                    actions
                }
                Step::Pkt(ts, channel, mut pkt, rewrite, actions) => {
                    let out = selector.select(&mut ctx, ts, channel, &mut pkt).map(|_| {
                        let seq = ctx.seq_rewrite.generate(pkt.seq as u64).expect("Should have seq") as u16;
                        let ts = ctx.ts_rewrite.generate(ts, pkt.ts as u64) as u32;
                        let (pic_id, tl0idx) = match &pkt.meta {
                            MediaMeta::Vp8 {
                                key: _,
                                sim: Some(Vp8Sim { picture_id, tl0_pic_idx, .. }),
                            } => (picture_id.unwrap(), tl0_pic_idx.unwrap()),
                            _ => panic!("Should not happen"),
                        };
                        (seq, ts, tl0idx, pic_id)
                    });
                    assert_eq!(out, rewrite, "input {:?}", pkt);
                    actions
                }
            };
            let mut actions = VecDeque::from(actions);

            loop {
                let action = selector.pop_action();
                let desired = actions.pop_front();
                assert_eq!(action, desired);
                if action.is_none() {
                    break;
                }
            }
        }
    }

    /// Test if first arrived pkt match filter
    #[test]
    fn start_low_layer() {
        let channel = 0;
        test(
            100,
            &[[50, 70, 100], [150, 200, 300]],
            vec![Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, 0, 0), Some((1, 0, 1, 1)), vec![])],
        )
    }

    /// Test if first arrived pkt not match filter
    #[test]
    fn start_high_layer() {
        let channel = 0;
        test(
            200, //this bitrate enough for select layer 1,1
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, 0, 0), None, vec![]),                 //sim1, temporal1
                Step::Pkt(0, channel, video_pkt(10, 0, true, None, 1, 0, false, 0, 0), Some((1, 0, 1, 1)), vec![]), //sim2, temporal1
                Step::Pkt(30, channel, video_pkt(1, 2700, false, None, 0, 1, false, 0, 1), None, vec![]),           //sim1, temporal2
                Step::Pkt(30, channel, video_pkt(11, 2700, false, None, 1, 1, false, 0, 1), Some((2, 2700, 1, 2)), vec![]), //sim2, temporal2
                Step::Pkt(30, channel, video_pkt(12, 2700, false, None, 1, 1, false, 0, 1), Some((3, 2700, 1, 2)), vec![]), //sim2, temporal2
                Step::Pkt(60, channel, video_pkt(2, 5400, false, None, 0, 2, false, 0, 2), None, vec![]),           //sim1, temporal3
                Step::Pkt(60, channel, video_pkt(13, 5400, false, None, 1, 2, false, 0, 2), None, vec![]),          //sim2, temporal3 => reject
                Step::Pkt(60, channel, video_pkt(14, 5400, false, None, 1, 2, false, 0, 2), None, vec![]),          //sim2, temporal3 => reject
                Step::Pkt(90, channel, video_pkt(3, 8100, false, None, 0, 0, true, 1, 3), None, vec![]),            //sim1, temporal1
                Step::Pkt(90, channel, video_pkt(15, 8100, false, None, 1, 0, false, 1, 3), Some((4, 8100, 2, 3)), vec![]), //sim2, temporal1
                Step::Pkt(120, channel, video_pkt(4, 10800, false, None, 0, 1, false, 1, 4), None, vec![]),         //sim1, temporal2
                Step::Pkt(120, channel, video_pkt(16, 10800, false, None, 1, 1, false, 1, 4), Some((5, 10800, 2, 4)), vec![]), //sim2, temporal2
                Step::Pkt(120, channel, video_pkt(17, 10800, false, None, 1, 1, false, 1, 4), Some((6, 10800, 2, 4)), vec![]), //sim2, temporal2
                Step::Pkt(150, channel, video_pkt(5, 13500, false, None, 0, 2, false, 1, 5), None, vec![]),         //sim1, temporal3
                Step::Pkt(150, channel, video_pkt(18, 13500, false, None, 1, 2, false, 1, 5), None, vec![]),        //sim2, temporal3 => reject
                Step::Pkt(150, channel, video_pkt(19, 13500, false, None, 1, 2, false, 1, 5), None, vec![]),        //sim2, temporal3 => reject
            ],
        )
    }

    /// Test with up temporal, need to wait pkt with layer_sync flag
    #[test]
    fn up_temporal_wait_layer_sync() {
        let channel = 0;
        test(
            50, // => only select layer 0, 0
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, 0, 0), Some((1, 0, 1, 1)), vec![]), //sim1, temporal1
                Step::Pkt(30, channel, video_pkt(1, 2700, false, None, 0, 1, false, 0, 1), None, vec![]),         //sim1, temporal2
                Step::Pkt(60, channel, video_pkt(2, 5400, false, None, 0, 2, false, 0, 2), None, vec![]),         //sim1, temporal3
                //now switch to 0, 2 => wait layer sync
                Step::Bitrate(60, 100, vec![]),
                Step::Pkt(90, channel, video_pkt(3, 8100, false, None, 0, 0, false, 1, 3), Some((2, 8100, 2, 2)), vec![]), //sim1, temporal1
                Step::Pkt(120, channel, video_pkt(4, 10800, false, None, 0, 1, false, 1, 4), None, vec![]),                //sim1, temporal2
                Step::Pkt(150, channel, video_pkt(5, 13500, false, None, 0, 2, false, 1, 5), None, vec![]),                //sim1, temporal3
                //has layer-sync => switch now
                Step::Pkt(180, channel, video_pkt(6, 16200, false, None, 0, 0, true, 2, 6), Some((3, 16200, 3, 3)), vec![]), //sim1, temporal1
                Step::Pkt(210, channel, video_pkt(7, 18900, false, None, 0, 1, false, 2, 7), Some((4, 18900, 3, 4)), vec![]), //sim1, temporal2
                Step::Pkt(240, channel, video_pkt(8, 21600, false, None, 0, 2, false, 2, 8), Some((5, 21600, 3, 5)), vec![]), //sim1, temporal3
            ],
        )
    }

    /// Test with down temporal
    #[test]
    fn down_temporal() {
        let channel = 0;
        test(
            100, // => select layer 0, 2
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, 0, 0), Some((1, 0, 1, 1)), vec![]), //sim1, temporal1
                Step::Pkt(30, channel, video_pkt(1, 2700, false, None, 0, 1, false, 0, 1), Some((2, 2700, 1, 2)), vec![]), //sim1, temporal2
                Step::Pkt(60, channel, video_pkt(2, 5400, false, None, 0, 2, false, 0, 2), Some((3, 5400, 1, 3)), vec![]), //sim1, temporal3
                //now switch to 0, 0
                Step::Bitrate(60, 50, vec![]),
                Step::Pkt(90, channel, video_pkt(3, 8100, false, None, 0, 0, false, 1, 3), Some((4, 8100, 2, 4)), vec![]), //sim1, temporal1
                Step::Pkt(120, channel, video_pkt(4, 10800, false, None, 0, 1, false, 1, 4), None, vec![]),                //sim1, temporal2
                Step::Pkt(150, channel, video_pkt(5, 13500, false, None, 0, 2, false, 1, 5), None, vec![]),                //sim1, temporal3
                Step::Pkt(180, channel, video_pkt(6, 16200, false, None, 0, 0, true, 2, 6), Some((5, 16200, 3, 5)), vec![]), //sim1, temporal1
                Step::Pkt(210, channel, video_pkt(7, 18900, false, None, 0, 1, false, 2, 7), None, vec![]),                //sim1, temporal2
                Step::Pkt(240, channel, video_pkt(8, 21600, false, None, 0, 2, false, 2, 8), None, vec![]),                //sim1, temporal3
            ],
        )
    }

    /// Test with up temporal, need to wait pkt with layer_sync flag
    #[test]
    fn up_spatial_wait_key_frame() {
        let channel = 0;
        test(
            50, // => only select layer 0, 0
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, 0, 0), Some((1, 0, 1, 1)), vec![]), //sim1, temporal1
                Step::Pkt(0, channel, video_pkt(10, 0, true, None, 1, 0, false, 0, 0), None, vec![]),             //sim2, temporal1
                Step::Pkt(30, channel, video_pkt(1, 2700, false, None, 0, 1, false, 0, 1), None, vec![]),         //sim1, temporal2
                Step::Pkt(30, channel, video_pkt(11, 2700, false, None, 1, 1, false, 0, 1), None, vec![]),        //sim2, temporal2
                Step::Pkt(30, channel, video_pkt(12, 2700, false, None, 1, 1, false, 0, 1), None, vec![]),        //sim2, temporal2
                Step::Pkt(60, channel, video_pkt(2, 5400, false, None, 0, 2, false, 0, 2), None, vec![]),         //sim1, temporal3
                Step::Pkt(60, channel, video_pkt(13, 5400, false, None, 1, 2, false, 0, 2), None, vec![]),        //sim2, temporal3
                Step::Pkt(60, channel, video_pkt(14, 5400, false, None, 1, 2, false, 0, 2), None, vec![]),        //sim2, temporal3
                //now target to 1, 2 => will switch to temporal 2 first util we have key-frame
                Step::Bitrate(90, 300, vec![Action::RequestKeyFrame]),
                Step::Pkt(90, channel, video_pkt(3, 8100, false, None, 0, 0, true, 1, 3), Some((2, 8100, 2, 2)), vec![Action::RequestKeyFrame]), //sim1, temporal1
                Step::Pkt(90, channel, video_pkt(15, 8100, false, None, 1, 0, false, 1, 3), None, vec![Action::RequestKeyFrame]),                //sim2, temporal1
                Step::Pkt(120, channel, video_pkt(4, 10800, false, None, 0, 1, false, 1, 4), Some((3, 10800, 2, 3)), vec![Action::RequestKeyFrame]), //sim1, temporal2
                Step::Pkt(120, channel, video_pkt(16, 10800, false, None, 1, 1, false, 1, 4), None, vec![Action::RequestKeyFrame]),              //sim2, temporal2
                Step::Pkt(120, channel, video_pkt(17, 10800, false, None, 1, 1, false, 1, 4), None, vec![Action::RequestKeyFrame]),              //sim2, temporal2
                Step::Pkt(150, channel, video_pkt(5, 13500, false, None, 0, 2, false, 1, 5), Some((4, 13500, 2, 4)), vec![Action::RequestKeyFrame]), //sim1, temporal3
                Step::Pkt(150, channel, video_pkt(18, 13500, false, None, 1, 2, false, 1, 5), None, vec![Action::RequestKeyFrame]),              //sim2, temporal3 => reject
                Step::Pkt(150, channel, video_pkt(19, 13500, false, None, 1, 2, false, 1, 5), None, vec![Action::RequestKeyFrame]),              //sim2, temporal3 => reject
                //now key-frame arrived  => switch to spatial 1
                Step::Pkt(180, channel, video_pkt(6, 16200, true, None, 0, 0, true, 2, 0), None, vec![]), //sim1, temporal1
                Step::Pkt(180, channel, video_pkt(20, 16200, true, None, 1, 0, false, 2, 0), Some((5, 16200, 3, 5)), vec![]), //sim2, temporal1
                Step::Pkt(210, channel, video_pkt(7, 18900, false, None, 0, 1, false, 2, 1), None, vec![]), //sim1, temporal2
                Step::Pkt(210, channel, video_pkt(21, 18900, false, None, 1, 1, false, 2, 1), Some((6, 18900, 3, 6)), vec![]), //sim2, temporal2
                Step::Pkt(210, channel, video_pkt(22, 18900, false, None, 1, 1, false, 2, 1), Some((7, 18900, 3, 6)), vec![]), //sim2, temporal2
                Step::Pkt(240, channel, video_pkt(8, 21600, false, None, 0, 2, false, 2, 2), None, vec![]), //sim1, temporal3
                Step::Pkt(240, channel, video_pkt(23, 21600, false, None, 1, 2, false, 2, 2), Some((8, 21600, 3, 7)), vec![]), //sim2, temporal3
                Step::Pkt(240, channel, video_pkt(24, 21600, false, None, 1, 2, false, 2, 2), Some((9, 21600, 3, 7)), vec![]), //sim2, temporal3
            ],
        )
    }

    /// Test with up temporal, need to wait pkt with layer_sync flag
    #[test]
    fn down_spatial_wait_key_frame() {
        let channel = 0;
        test(
            300, // => select layer 1, 2
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, 0, 0), None, vec![]),                 //sim1, temporal1
                Step::Pkt(0, channel, video_pkt(10, 0, true, None, 1, 0, false, 0, 0), Some((1, 0, 1, 1)), vec![]), //sim2, temporal1
                Step::Pkt(30, channel, video_pkt(1, 2700, false, None, 0, 1, false, 0, 1), None, vec![]),           //sim1, temporal2
                Step::Pkt(30, channel, video_pkt(11, 2700, false, None, 1, 1, false, 0, 1), Some((2, 2700, 1, 2)), vec![]), //sim2, temporal2
                Step::Pkt(30, channel, video_pkt(12, 2700, false, None, 1, 1, false, 0, 1), Some((3, 2700, 1, 2)), vec![]), //sim2, temporal2
                Step::Pkt(60, channel, video_pkt(2, 5400, false, None, 0, 2, false, 0, 2), None, vec![]),           //sim1, temporal3
                Step::Pkt(60, channel, video_pkt(13, 5400, false, None, 1, 2, false, 0, 2), Some((4, 5400, 1, 3)), vec![]), //sim2, temporal3
                Step::Pkt(60, channel, video_pkt(14, 5400, false, None, 1, 2, false, 0, 2), Some((5, 5400, 1, 3)), vec![]), //sim2, temporal3
                //now target to 0, 2 => will switch to temporal 0 first util we have key-frame
                Step::Bitrate(90, 100, vec![Action::RequestKeyFrame]),
                Step::Pkt(90, channel, video_pkt(3, 8100, false, None, 0, 0, true, 1, 3), None, vec![Action::RequestKeyFrame]), //sim1, temporal1
                Step::Pkt(90, channel, video_pkt(15, 8100, false, None, 1, 0, false, 1, 3), Some((6, 8100, 2, 4)), vec![Action::RequestKeyFrame]), //sim2, temporal1
                Step::Pkt(120, channel, video_pkt(4, 10800, false, None, 0, 1, false, 1, 4), None, vec![Action::RequestKeyFrame]), //sim1, temporal2
                Step::Pkt(120, channel, video_pkt(16, 10800, false, None, 1, 1, false, 1, 4), None, vec![Action::RequestKeyFrame]), //sim2, temporal2
                Step::Pkt(120, channel, video_pkt(17, 10800, false, None, 1, 1, false, 1, 4), None, vec![Action::RequestKeyFrame]), //sim2, temporal2
                Step::Pkt(150, channel, video_pkt(5, 13500, false, None, 0, 2, false, 1, 5), None, vec![Action::RequestKeyFrame]), //sim1, temporal3
                Step::Pkt(150, channel, video_pkt(18, 13500, false, None, 1, 2, false, 1, 5), None, vec![Action::RequestKeyFrame]), //sim2, temporal3 => reject
                Step::Pkt(150, channel, video_pkt(19, 13500, false, None, 1, 2, false, 1, 5), None, vec![Action::RequestKeyFrame]), //sim2, temporal3 => reject
                //now key-frame arrived  => switch to spatial 0
                Step::Pkt(180, channel, video_pkt(6, 16200, true, None, 0, 0, true, 2, 0), Some((7, 16200, 3, 5)), vec![]), //sim1, temporal1
                Step::Pkt(180, channel, video_pkt(20, 16200, true, None, 1, 0, false, 2, 0), None, vec![]),                 //sim2, temporal1
                Step::Pkt(210, channel, video_pkt(7, 18900, false, None, 0, 1, false, 2, 1), Some((8, 18900, 3, 6)), vec![]), //sim1, temporal2
                Step::Pkt(210, channel, video_pkt(21, 18900, false, None, 1, 1, false, 2, 1), None, vec![]),                //sim2, temporal2
                Step::Pkt(210, channel, video_pkt(22, 18900, false, None, 1, 1, false, 2, 1), None, vec![]),                //sim2, temporal2
                Step::Pkt(240, channel, video_pkt(8, 21600, false, None, 0, 2, false, 2, 2), Some((9, 21600, 3, 7)), vec![]), //sim1, temporal3
                Step::Pkt(240, channel, video_pkt(23, 21600, false, None, 1, 2, false, 2, 2), None, vec![]),                //sim2, temporal3
                Step::Pkt(240, channel, video_pkt(24, 21600, false, None, 1, 2, false, 2, 2), None, vec![]),                //sim2, temporal3
            ],
        )
    }
}
