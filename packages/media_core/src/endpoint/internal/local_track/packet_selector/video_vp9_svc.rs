//! Vp9 svc selector
//!
//! This selector take care switch to best layer based on target bitrate and source layers bitrate.
//!
//! In order to switch, it will have own state pic_id_rewrite to ensure output stream
//! is not disconnect, and also call drop_value and reinit on shared state (seq_rewrite, ts_rewrite) when
//! it switch to other spatial layer.
//!
//! Note that, in svc stream, all spatial layers have continuous seq, ts.
//!
//! Vp9 have 2 mode: k-svc and full-svc
//!
//! - K-SVC: we need key-frame for up and down layer
//! - Full-SVCL we only need key-frame for up, and only end-frame flag for down layer

use std::collections::VecDeque;

use media_server_protocol::media::{MediaLayerSelection, MediaLayersBitrate, MediaMeta, MediaPacket};
use media_server_utils::SeqRewrite;

use super::{Action, VideoSelector, VideoSelectorCtx};

const PIC_ID_MAX: u64 = 1 << 15;

#[derive(Default)]
pub struct Ctx {
    pic_id_rewrite: SeqRewrite<PIC_ID_MAX, 60>,
}

pub struct Selector {
    k_svc: bool,
    bitrate_kbps: u16,
    layers: MediaLayersBitrate,
    current: Option<MediaLayerSelection>,
    target: Option<MediaLayerSelection>,
    queue: VecDeque<Action>,
    //for alert previous frame end then we can switch layer if need
    pre_end_frame: bool,
    limit: (u8, u8),
}

impl Selector {
    pub fn new(k_svc: bool, bitrate: u64, layers: MediaLayersBitrate, limit: (u8, u8)) -> Self {
        let bitrate_kbps = (bitrate / 1000) as u16;
        let (max_spatial, max_temporal) = limit;
        let target = layers.select_layer(bitrate_kbps, max_spatial, max_temporal);

        log::info!("[Vp9SvcSelector] create with bitrate {bitrate_kbps} kbps, layers {:?} => init target {:?}", layers, target);

        Self {
            k_svc,
            bitrate_kbps,
            layers,
            current: None,
            target,
            queue: VecDeque::new(),
            pre_end_frame: false,
            limit: (max_spatial, max_temporal),
        }
    }

    fn select_layer(&mut self) {
        let target = self.layers.select_layer(self.bitrate_kbps, self.limit.0, self.limit.1);
        if target != self.target {
            log::info!("[Vp9SvcSelector] bitrate {} kbps, layers {:?} => changed target to {:?}", self.bitrate_kbps, self.layers, target);
            self.target = target;

            if let Some(target) = &self.target {
                if let Some(current) = &self.current {
                    if self.k_svc || target.spatial > current.spatial {
                        log::info!("[Vp9SvcSelector] switch to up spatial layer in k-svc mode => request key frame");
                        self.queue.push_back(Action::RequestKeyFrame);
                    }
                } else {
                    log::info!("[Vp9SvcSelector] switch to new spatial layer from pause state => request key frame");
                    self.queue.push_back(Action::RequestKeyFrame);
                }
            }
        }
    }

    fn try_switch(&mut self, ctx: &mut VideoSelectorCtx, pkt: &mut MediaPacket) {
        if self.target == self.current {
            return;
        }
        if let MediaMeta::Vp9 { key, profile: _, svc: Some(svc) } = &mut pkt.meta {
            match (&mut self.current, &self.target) {
                (Some(current), Some(target)) => {
                    //need switch to temporal layer only
                    if target.spatial == current.spatial {
                        //change temporal
                        if target.temporal > current.temporal {
                            //up temporal => need wait switching_point and pre frame is end
                            if svc.spatial == current.spatial && svc.temporal > current.temporal && svc.switching_point && self.pre_end_frame {
                                log::info!("[Vp9SvcSelector] up temporal {},{} => {},{}", current.spatial, current.temporal, target.spatial, target.temporal);
                                current.temporal = target.temporal;
                            }
                        } else if target.temporal < current.temporal {
                            //down temporal => need wait end_frame
                            if self.pre_end_frame {
                                log::info!("[Vp9SvcSelector] down temporal {},{} => {},{}", current.spatial, current.temporal, target.spatial, target.temporal);
                                current.temporal = target.temporal;
                            }
                        }
                    } else if target.spatial < current.spatial {
                        // down spatial => need wait key-frame
                        // first we allway down temporal for trying reduce bandwidth
                        if current.temporal != 0 && self.pre_end_frame {
                            log::info!("[Vp9SvcSelector] down spatial then down temporal from {} => 0", current.temporal);
                            current.temporal = 0;
                        }
                        // In K-SVC we must wait for a keyframe.
                        // In full SVC we do not need a keyframe.
                        if (self.k_svc && *key) || (!self.k_svc && self.pre_end_frame) {
                            log::info!("[Vp9SvcSelector] down {},{} => {},{} with key", current.spatial, current.temporal, target.spatial, target.temporal);
                            // with other spatial we have difference tl0xidx and pic_id offset
                            // therefore we need reinit both tl0idx and pic_id
                            ctx.vp9_ctx.pic_id_rewrite.reinit();
                            ctx.seq_rewrite.reinit();
                            ctx.ts_rewrite.reinit();
                            current.spatial = target.spatial;
                            current.temporal = target.temporal;
                        } else if self.k_svc {
                            self.queue.push_back(Action::RequestKeyFrame);
                        }
                    } else if target.spatial > current.spatial {
                        // up spatial => need wait key-frame
                        // first we try to up temporal for trying increase bandwidth
                        if svc.spatial == current.spatial && svc.temporal > current.temporal && current.temporal != 2 && svc.switching_point && self.pre_end_frame {
                            log::info!("[Vp9SvcSelector] up spatial then up temporal from {} => 2 before key arrived", current.temporal);
                            current.temporal = 2;
                        }

                        if *key {
                            log::info!("[Vp9SvcSelector] up {},{} => {},{} with key-frame", current.spatial, current.temporal, target.spatial, target.temporal);
                            // with other spatial we have difference tl0xidx and pic_id offset
                            // therefore we need reinit both tl0idx and pic_id
                            ctx.vp9_ctx.pic_id_rewrite.reinit();
                            ctx.seq_rewrite.reinit();
                            ctx.ts_rewrite.reinit();
                            current.spatial = target.spatial;
                            current.temporal = target.temporal;
                        } else {
                            self.queue.push_back(Action::RequestKeyFrame);
                        }
                    }
                }
                (Some(current), None) => {
                    // need pause
                    if self.pre_end_frame {
                        log::info!("[Vp9SvcSelector] end-frame => pause from {},{}", current.spatial, current.temporal);
                        self.current = None;
                    }
                }
                (None, Some(target)) => {
                    // need resume or start => need wait key_frame
                    if *key {
                        log::info!("[Vp9SvcSelector] resume to {},{} with key", target.spatial, target.temporal);
                        // with other spatial we have difference tl0xidx and pic_id offset
                        // therefore we need reinit both tl0idx and pic_id
                        ctx.vp9_ctx.pic_id_rewrite.reinit();
                        self.current = Some(target.clone());
                    }
                }
                (None, None) => {
                    //reject
                }
            }

            self.pre_end_frame = svc.end_frame;
        }
    }

    fn is_allow(&mut self, ctx: &mut VideoSelectorCtx, pkt: &mut MediaPacket) -> Option<()> {
        let current = self.current.as_ref()?;
        match &mut pkt.meta {
            MediaMeta::Vp9 { key: _, profile: _, svc: Some(svc) } => {
                if svc.spatial <= current.spatial && svc.temporal <= current.temporal {
                    log::trace!(
                        "[Vp9SvcSelector] allow {} {}, seq {}, ts {}, marker {}, pic_id {:?}",
                        svc.spatial,
                        svc.temporal,
                        pkt.seq,
                        pkt.ts,
                        pkt.marker,
                        svc.picture_id
                    );

                    if let Some(pic_id) = svc.picture_id {
                        svc.picture_id = Some(ctx.vp9_ctx.pic_id_rewrite.generate(pic_id as u64)? as u16);
                    }

                    if svc.spatial == current.spatial && svc.end_frame {
                        pkt.marker = true;
                    }

                    Some(())
                } else {
                    log::trace!("[Vp9SvcSelector] reject {} {}, seq {}, ts {}", svc.spatial, svc.temporal, pkt.seq, pkt.ts);
                    // with TID (temporal) = 0, which never drop
                    if let Some(pic_id) = svc.picture_id {
                        ctx.vp9_ctx.pic_id_rewrite.drop_value(pic_id as u64);
                    }

                    ctx.seq_rewrite.drop_value(pkt.seq as u64);

                    None
                }
            }
            _ => None,
        }
    }
}

impl VideoSelector for Selector {
    fn on_init(&mut self, ctx: &mut VideoSelectorCtx, _now_ms: u64) {
        ctx.vp9_ctx.pic_id_rewrite.reinit();
    }

    fn on_tick(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64) {}

    fn set_target_bitrate(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64, bitrate: u64) {
        let bitrate_kbps = (bitrate / 1000) as u16;
        self.bitrate_kbps = bitrate_kbps;
        self.select_layer();
    }

    fn set_limit_layer(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64, max_spatial: u8, max_temporal: u8) {
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
    use media_server_protocol::media::{MediaKind, MediaLayerBitrate, MediaLayersBitrate, MediaMeta, MediaPacket, Vp9Profile, Vp9Svc};

    fn layers_bitrate(layers: &[[u16; 3]]) -> MediaLayersBitrate {
        let mut res = MediaLayersBitrate::default();
        for (index, layer) in layers.iter().enumerate() {
            res.set_layer(index, MediaLayerBitrate::new(layer));
        }
        res
    }

    fn video_pkt(seq: u16, ts: u32, key: bool, layers: Option<&[[u16; 3]]>, spatial: u8, temporal: u8, switching_point: bool, end_frame: bool, pic_id: u16) -> MediaPacket {
        MediaPacket {
            ts,
            seq,
            marker: true,
            nackable: false,
            layers: layers.map(layers_bitrate),
            meta: MediaMeta::Vp9 {
                key,
                profile: Vp9Profile::P0,
                svc: Some(Vp9Svc {
                    picture_id: Some(pic_id),
                    spatial,
                    temporal,
                    switching_point,
                    end_frame,
                    begin_frame: false,
                    spatial_layers: None,
                    predicted_frame: false,
                }),
            },
            data: vec![1, 2, 3],
        }
    }

    #[derive(Debug, Clone)]
    enum Step {
        Bitrate(u64, u64, Vec<Action>),
        Pkt(u64, u64, MediaPacket, Option<(u16, u32, u16)>, Vec<Action>),
    }

    fn test(bitrate: u64, layers: &[[u16; 3]], steps: Vec<Step>) {
        let mut ctx = VideoSelectorCtx::new(MediaKind::Video);
        ctx.seq_rewrite.reinit();
        let mut selector = Selector::new(false, bitrate * 1000, layers_bitrate(&layers), (2, 2));
        selector.on_init(&mut ctx, 0);

        for step in steps {
            let actions = match step.clone() {
                Step::Bitrate(ts, bitrate, actions) => {
                    selector.set_target_bitrate(&mut ctx, ts, bitrate * 1000);
                    actions
                }
                Step::Pkt(ts, channel, mut pkt, rewrite, actions) => {
                    let out = selector.select(&mut ctx, ts, channel, &mut pkt).map(|_| {
                        let seq = ctx.seq_rewrite.generate(pkt.seq as u64).expect("Should have seq") as u16;
                        let ts = ctx.ts_rewrite.generate(ts, pkt.ts as u64) as u32;
                        let pic_id = match &pkt.meta {
                            MediaMeta::Vp9 {
                                svc: Some(Vp9Svc { picture_id, .. }), ..
                            } => picture_id.expect("Should have pic_id"),
                            _ => panic!("Should not happen"),
                        };
                        (seq, ts, pic_id)
                    });
                    assert_eq!(out, rewrite, "input {:?}", pkt);
                    actions
                }
            };
            let mut actions = VecDeque::from(actions);

            loop {
                let action = selector.pop_action();
                let desired = actions.pop_front();
                assert_eq!(action, desired, "step {:?}", step);
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
            vec![Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, true, 0), Some((1, 0, 1)), vec![])],
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
                //
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, true, 0), Some((1, 0, 1)), vec![]), //svc1, temporal1
                Step::Pkt(0, channel, video_pkt(1, 0, true, None, 1, 0, false, true, 0), Some((2, 0, 1)), vec![]), //svc2, temporal1
                Step::Pkt(30, channel, video_pkt(2, 2700, false, None, 0, 1, false, true, 1), Some((3, 2700, 2)), vec![]), //svc1, temporal2
                Step::Pkt(30, channel, video_pkt(3, 2700, false, None, 1, 1, false, false, 1), Some((4, 2700, 2)), vec![]), //svc2, temporal2
                Step::Pkt(30, channel, video_pkt(4, 2700, false, None, 1, 1, false, true, 1), Some((5, 2700, 2)), vec![]), //svc2, temporal2
                Step::Pkt(60, channel, video_pkt(5, 5400, false, None, 0, 2, false, true, 2), None, vec![]),      //svc1, temporal3
                Step::Pkt(60, channel, video_pkt(6, 5400, false, None, 1, 2, false, false, 2), None, vec![]),     //svc2, temporal3 => reject
                Step::Pkt(60, channel, video_pkt(7, 5400, false, None, 1, 2, false, true, 2), None, vec![]),      //svc2, temporal3 => reject
                //
                Step::Pkt(90, channel, video_pkt(8, 8100, false, None, 0, 0, true, true, 3), Some((6, 8100, 3)), vec![]), //svc1, temporal1
                Step::Pkt(90, channel, video_pkt(9, 8100, false, None, 1, 0, false, true, 3), Some((7, 8100, 3)), vec![]), //svc2, temporal1
                Step::Pkt(120, channel, video_pkt(10, 10800, false, None, 0, 1, false, true, 4), Some((8, 10800, 4)), vec![]), //svc1, temporal2
                Step::Pkt(120, channel, video_pkt(11, 10800, false, None, 1, 1, false, false, 4), Some((9, 10800, 4)), vec![]), //svc2, temporal2
                Step::Pkt(120, channel, video_pkt(12, 10800, false, None, 1, 1, false, true, 4), Some((10, 10800, 4)), vec![]), //svc2, temporal2
                Step::Pkt(150, channel, video_pkt(13, 13500, false, None, 0, 2, false, true, 5), None, vec![]),           //svc1, temporal3
                Step::Pkt(150, channel, video_pkt(14, 13500, false, None, 1, 2, false, false, 5), None, vec![]),          //svc2, temporal3 => reject
                Step::Pkt(150, channel, video_pkt(15, 13500, false, None, 1, 2, false, true, 5), None, vec![]),           //svc2, temporal3 => reject
            ],
        )
    }

    /// Test with up temporal, need to wait current frame end and switching point
    #[test]
    fn up_temporal_wait_layer_sync() {
        let channel = 0;
        test(
            50, // => only select layer 0, 0
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, true, 0), Some((1, 0, 1)), vec![]), //svc1, temporal1
                Step::Pkt(30, channel, video_pkt(1, 2700, false, None, 0, 1, false, true, 1), None, vec![]),      //svc1, temporal2
                Step::Pkt(60, channel, video_pkt(2, 5400, false, None, 0, 2, false, true, 2), None, vec![]),      //svc1, temporal3
                //now switch to 0, 2 => wait layer sync
                Step::Bitrate(60, 100, vec![]),
                Step::Pkt(90, channel, video_pkt(3, 8100, false, None, 0, 0, false, true, 3), Some((2, 8100, 2)), vec![]), //svc1, temporal1
                Step::Pkt(120, channel, video_pkt(4, 10800, false, None, 0, 1, false, true, 4), None, vec![]),             //svc1, temporal2
                Step::Pkt(150, channel, video_pkt(5, 13500, false, None, 0, 2, false, true, 5), None, vec![]),             //svc1, temporal3
                //has switching-point => switch now
                Step::Pkt(180, channel, video_pkt(6, 16200, false, None, 0, 0, true, true, 6), Some((3, 16200, 3)), vec![]), //svc1, temporal1
                Step::Pkt(210, channel, video_pkt(7, 18900, false, None, 0, 1, true, true, 7), Some((4, 18900, 4)), vec![]), //svc1, temporal2
                Step::Pkt(240, channel, video_pkt(8, 21600, false, None, 0, 2, true, true, 8), Some((5, 21600, 5)), vec![]), //svc1, temporal3
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
                Step::Pkt(0, channel, video_pkt(0, 0, true, None, 0, 0, true, true, 0), Some((1, 0, 1)), vec![]), //svc1, temporal1
                Step::Pkt(30, channel, video_pkt(1, 2700, false, None, 0, 1, false, true, 1), Some((2, 2700, 2)), vec![]), //svc1, temporal2
                Step::Pkt(60, channel, video_pkt(2, 5400, false, None, 0, 2, false, true, 2), Some((3, 5400, 3)), vec![]), //svc1, temporal3
                //now switch to 0, 0
                Step::Bitrate(60, 50, vec![]),
                Step::Pkt(90, channel, video_pkt(3, 8100, false, None, 0, 0, false, true, 3), Some((4, 8100, 4)), vec![]), //svc1, temporal1
                Step::Pkt(120, channel, video_pkt(4, 10800, false, None, 0, 1, false, true, 4), None, vec![]),             //svc1, temporal2
                Step::Pkt(150, channel, video_pkt(5, 13500, false, None, 0, 2, false, true, 5), None, vec![]),             //svc1, temporal3
                Step::Pkt(180, channel, video_pkt(6, 16200, false, None, 0, 0, true, true, 6), Some((5, 16200, 5)), vec![]), //svc1, temporal1
                Step::Pkt(210, channel, video_pkt(7, 18900, false, None, 0, 1, false, true, 7), None, vec![]),             //svc1, temporal2
                Step::Pkt(240, channel, video_pkt(8, 21600, false, None, 0, 2, false, true, 8), None, vec![]),             //svc1, temporal3
            ],
        )
    }

    /// Test with up temporal, need to wait pkt with layer_sync flag
    #[test]
    fn up_spatial_wait_key_frame() {
        let c = 0;
        test(
            50, // => only select layer 0, 0
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, c, video_pkt(0, 0, true, None, 0, 0, false, true, 0), Some((1, 0, 1)), vec![]), //svc1, temporal1
                Step::Pkt(0, c, video_pkt(1, 0, true, None, 1, 0, false, true, 0), None, vec![]),            //svc2, temporal1
                Step::Pkt(30, c, video_pkt(2, 2700, false, None, 0, 1, false, true, 1), None, vec![]),       //svc1, temporal2
                Step::Pkt(30, c, video_pkt(3, 2700, false, None, 1, 1, false, false, 1), None, vec![]),      //svc2, temporal2
                Step::Pkt(30, c, video_pkt(4, 2700, false, None, 1, 1, false, true, 1), None, vec![]),       //svc2, temporal2
                Step::Pkt(60, c, video_pkt(5, 5400, false, None, 0, 2, false, true, 2), None, vec![]),       //svc1, temporal3
                Step::Pkt(60, c, video_pkt(6, 5400, false, None, 1, 2, false, false, 2), None, vec![]),      //svc2, temporal3
                Step::Pkt(60, c, video_pkt(7, 5400, false, None, 1, 2, false, true, 2), None, vec![]),       //svc2, temporal3
                //now target to 1, 2 => will switch to temporal 2 first util we have key-frame
                Step::Bitrate(90, 300, vec![Action::RequestKeyFrame]),
                Step::Pkt(90, c, video_pkt(8, 8100, false, None, 0, 0, true, true, 3), Some((2, 8100, 2)), vec![Action::RequestKeyFrame]), //svc1, temporal1
                Step::Pkt(90, c, video_pkt(9, 8100, false, None, 1, 0, true, true, 3), None, vec![Action::RequestKeyFrame]),               //svc2, temporal1
                Step::Pkt(120, c, video_pkt(10, 10800, false, None, 0, 1, true, true, 4), Some((3, 10800, 3)), vec![Action::RequestKeyFrame]), //svc1, temporal2
                Step::Pkt(120, c, video_pkt(11, 10800, false, None, 1, 1, true, false, 4), None, vec![Action::RequestKeyFrame]),           //svc2, temporal2
                Step::Pkt(120, c, video_pkt(12, 10800, false, None, 1, 1, true, true, 4), None, vec![Action::RequestKeyFrame]),            //svc2, temporal2
                Step::Pkt(150, c, video_pkt(13, 13500, false, None, 0, 2, true, true, 5), Some((4, 13500, 4)), vec![Action::RequestKeyFrame]), //svc1, temporal3
                Step::Pkt(150, c, video_pkt(14, 13500, false, None, 1, 2, true, false, 5), None, vec![Action::RequestKeyFrame]),           //svc2, temporal3 => reject
                Step::Pkt(150, c, video_pkt(15, 13500, false, None, 1, 2, true, true, 5), None, vec![Action::RequestKeyFrame]),            //svc2, temporal3 => reject
                //now key-frame arrived  => switch to spatial 1
                Step::Pkt(180, c, video_pkt(16, 16200, true, None, 0, 0, false, true, 6), Some((5, 16200, 5)), vec![]), //svc1, temporal1
                Step::Pkt(180, c, video_pkt(17, 16200, true, None, 1, 0, false, true, 6), Some((6, 16200, 5)), vec![]), //svc2, temporal1
                Step::Pkt(210, c, video_pkt(18, 18900, false, None, 0, 1, false, true, 7), Some((7, 18900, 6)), vec![]), //svc1, temporal2
                Step::Pkt(210, c, video_pkt(19, 18900, false, None, 1, 1, false, false, 7), Some((8, 18900, 6)), vec![]), //svc2, temporal2
                Step::Pkt(210, c, video_pkt(20, 18900, false, None, 1, 1, false, true, 7), Some((9, 18900, 6)), vec![]), //svc2, temporal2
                Step::Pkt(240, c, video_pkt(21, 21600, false, None, 0, 2, false, true, 8), Some((10, 21600, 7)), vec![]), //svc1, temporal3
                Step::Pkt(240, c, video_pkt(22, 21600, false, None, 1, 2, false, false, 8), Some((11, 21600, 7)), vec![]), //svc2, temporal3
                Step::Pkt(240, c, video_pkt(23, 21600, false, None, 1, 2, false, true, 8), Some((12, 21600, 7)), vec![]), //svc2, temporal3
            ],
        )
    }

    /// Test with down spatial, need to wait key-frame for non k-svc type => dont need wait key-frame, only endframe
    #[test]
    fn down_spatial_wait_end_frame() {
        let c = 0;
        test(
            300, // => select layer 1, 2
            &[[50, 70, 100], [150, 200, 300]],
            vec![
                Step::Pkt(0, c, video_pkt(0, 0, true, None, 0, 0, true, true, 0), Some((1, 0, 1)), vec![]),  //svc1, temporal1
                Step::Pkt(0, c, video_pkt(1, 0, true, None, 1, 0, false, true, 0), Some((2, 0, 1)), vec![]), //svc2, temporal1
                Step::Pkt(30, c, video_pkt(2, 2700, false, None, 0, 1, false, true, 1), Some((3, 2700, 2)), vec![]), //svc1, temporal2
                Step::Pkt(30, c, video_pkt(3, 2700, false, None, 1, 1, false, false, 1), Some((4, 2700, 2)), vec![]), //svc2, temporal2
                Step::Pkt(30, c, video_pkt(4, 2700, false, None, 1, 1, false, true, 1), Some((5, 2700, 2)), vec![]), //svc2, temporal2
                Step::Pkt(60, c, video_pkt(5, 5400, false, None, 0, 2, false, true, 2), Some((6, 5400, 3)), vec![]), //svc1, temporal3
                Step::Pkt(60, c, video_pkt(6, 5400, false, None, 1, 2, false, false, 2), Some((7, 5400, 3)), vec![]), //svc2, temporal3
                Step::Pkt(60, c, video_pkt(7, 5400, false, None, 1, 2, false, true, 2), Some((8, 5400, 3)), vec![]), //svc2, temporal3
                //now target to 0, 2 => will switch to temporal 0 first util we have end-frame
                Step::Bitrate(90, 100, vec![]),
                Step::Pkt(90, c, video_pkt(8, 8100, false, None, 0, 0, true, true, 3), Some((9, 8100, 4)), vec![]), //svc1, temporal1
                //now end-frame arrived  => switch to spatial 0, 2
                Step::Pkt(90, c, video_pkt(9, 8100, false, None, 1, 0, false, true, 3), None, vec![]), //svc2, temporal1
                //
                Step::Pkt(120, c, video_pkt(10, 10800, false, None, 0, 1, false, true, 4), Some((10, 10800, 5)), vec![]), //svc1, temporal2
                Step::Pkt(120, c, video_pkt(11, 10800, false, None, 1, 1, false, false, 4), None, vec![]),                //svc2, temporal2
                Step::Pkt(120, c, video_pkt(12, 10800, false, None, 1, 1, false, true, 4), None, vec![]),                 //svc2, temporal2
                Step::Pkt(150, c, video_pkt(13, 13500, false, None, 0, 2, false, true, 5), Some((11, 13500, 6)), vec![]), //svc1, temporal3
                Step::Pkt(150, c, video_pkt(14, 13500, false, None, 1, 2, false, false, 5), None, vec![]),                //svc2, temporal3 => reject
                Step::Pkt(150, c, video_pkt(15, 13500, false, None, 1, 2, false, true, 5), None, vec![]),                 //svc2, temporal3 => reject
            ],
        )
    }
}
