use std::collections::VecDeque;

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
}

impl Selector {
    pub fn new(bitrate: u64, layers: MediaLayersBitrate) -> Self {
        let bitrate_kbps = (bitrate / 1000) as u16;
        let target = layers.select_layer(bitrate_kbps);

        Self {
            bitrate_kbps,
            layers,
            current: None,
            target,
            queue: VecDeque::new(),
        }
    }

    fn select_layer(&mut self) {
        let target = self.layers.select_layer(self.bitrate_kbps);
        if target != self.target {
            log::info!("[Vp8SimSelector] bitrate {} kbps, layers {:?} => changed target to {:?}", self.bitrate_kbps, self.layers, target);
            self.target = target;

            if let Some(target) = &self.target {
                if self.current == None || target.spatial != self.current.as_ref().expect("Should have").spatial {
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
                    //need switch to new layer
                    if target.spatial == current.spatial {
                        //change temporal
                        if target.temporal > current.temporal {
                            //up temporal => need wait layer_sync
                            if sim.spatial == current.spatial && sim.temporal <= target.temporal && sim.layer_sync {
                                log::info!("[Vp8SimSelector] up temporal {} => {}", current.temporal, target.temporal);
                                current.temporal = target.temporal;
                            }
                        } else if target.temporal < current.temporal {
                            //down temporal => do now
                            log::info!("[Vp8SimSelector] down temporal {} => {}", current.temporal, target.temporal);
                            current.temporal = target.temporal;
                        }
                    } else if target.spatial < current.spatial {
                        //down spatial => need wait key-frame
                        //first we allway down temporal for trying reduce bandwidth
                        if current.temporal != 0 {
                            log::info!("[Vp8SimSelector] down spatial then down temporal from {} => 0", current.temporal);
                            current.temporal = 0;
                        }
                        if sim.spatial == target.spatial && *key {
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
                    } else if target.spatial > current.spatial {
                        //up spatial => need wait key-frame
                        //first we try to up temporal for trying increase bandwidth
                        if sim.spatial == current.spatial && current.temporal != 2 && sim.layer_sync {
                            log::info!("[Vp8SimSelector] up spatial then up temporal from {} => 2 with layer_sync", current.temporal);
                            current.temporal = 2;
                        }

                        if sim.spatial == target.spatial && *key {
                            log::info!("[Vp8SimSelector] up {},{} => {},{} with key", current.spatial, current.temporal, target.spatial, target.temporal);
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
                (Some(current), None) => {
                    //need pause
                    //TODO wait current frame finished for avoiding interrupt client
                    log::info!("[Vp8SimSelector] pause from {},{}", current.spatial, current.temporal);
                    self.current = None;
                }
                (None, Some(target)) => {
                    //need resume or start => need wait key_frame
                    if sim.spatial == target.spatial && *key {
                        log::info!("[Vp8SimSelector] resume to {},{} with key", target.spatial, target.temporal);
                        // with other spatial we have difference tl0xidx and pic_id offset
                        // therefore we need reinit both tl0idx and pic_id
                        ctx.vp8_ctx.tl0idx_rewrite.reinit();
                        ctx.vp8_ctx.pic_id_rewrite.reinit();
                        self.current = Some(target.clone());
                    }
                }
                (None, None) => {
                    //reject
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
    fn on_init(&mut self, ctx: &mut VideoSelectorCtx, now_ms: u64) {
        ctx.vp8_ctx.pic_id_rewrite.reinit();
        ctx.vp8_ctx.tl0idx_rewrite.reinit();
    }

    fn on_tick(&mut self, ctx: &mut VideoSelectorCtx, _now_ms: u64) {}

    fn set_target_bitrate(&mut self, ctx: &mut VideoSelectorCtx, _now_ms: u64, bitrate: u64) {
        let bitrate_kbps = (bitrate / 1000) as u16;
        self.bitrate_kbps = bitrate_kbps;
        self.select_layer();
    }

    fn selector(&mut self, ctx: &mut VideoSelectorCtx, _now_ms: u64, _channel: u64, pkt: &mut MediaPacket) -> Option<()> {
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
