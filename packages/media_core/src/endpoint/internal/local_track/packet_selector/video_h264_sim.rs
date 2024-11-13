use std::{cmp::Ordering, collections::VecDeque};

use media_server_protocol::media::{MediaLayersBitrate, MediaMeta, MediaPacket};

use super::{Action, VideoSelector, VideoSelectorCtx};

pub struct Selector {
    bitrate_kbps: u16,
    layers: MediaLayersBitrate,
    current: Option<u8>,
    target: Option<u8>,
    queue: VecDeque<Action>,
    limit: (u8, u8),
}

impl Selector {
    pub fn new(bitrate: u64, layers: MediaLayersBitrate, limit: (u8, u8)) -> Self {
        let bitrate_kbps = (bitrate / 1000) as u16;
        let (max_spatial, max_temporal) = limit;
        let target = layers.select_layer(bitrate_kbps, max_spatial, max_temporal);

        log::info!("[H264SimSelector] create with bitrate {bitrate_kbps} kbps, layers {:?} => init target {:?}", layers, target);

        Self {
            bitrate_kbps,
            layers,
            current: None,
            target: target.map(|t| t.spatial),
            queue: VecDeque::new(),
            limit: (max_spatial, max_temporal),
        }
    }

    fn select_layer(&mut self) {
        let target = self.layers.select_layer(self.bitrate_kbps, self.limit.0, self.limit.1).map(|t| t.spatial);
        if target != self.target {
            log::info!("[H264SimSelector] bitrate {} kbps, layers {:?} => changed target to {:?}", self.bitrate_kbps, self.layers, target);
            self.target = target;

            if let Some(target) = self.target {
                if self.current.is_none() || target != self.current.expect("Should have") {
                    log::info!("[H264SimSelector] switch to new spatial layer => request key frame");
                    self.queue.push_back(Action::RequestKeyFrame);
                }
            }
        }
    }

    fn try_switch(&mut self, ctx: &mut VideoSelectorCtx, pkt: &mut MediaPacket) {
        if self.target == self.current {
            return;
        }
        if let MediaMeta::H264 { key, profile: _, sim: Some(_sim) } = &mut pkt.meta {
            match (self.current, self.target) {
                (Some(current), Some(target)) => {
                    match target.cmp(&current) {
                        Ordering::Less => {
                            // down spatial => need wait key-frame
                            if *key {
                                log::info!("[H264SimSelector] down {} => {} with key", current, target);
                                ctx.seq_rewrite.reinit();
                                ctx.ts_rewrite.reinit();
                                self.current = self.target;
                            } else {
                                self.queue.push_back(Action::RequestKeyFrame);
                            }
                        }
                        Ordering::Greater => {
                            // up spatial => need wait key-frame
                            if *key {
                                log::info!("[H264SimSelector] up {} => {} with key", current, target);
                                ctx.seq_rewrite.reinit();
                                ctx.ts_rewrite.reinit();
                                self.current = Some(target);
                            } else {
                                self.queue.push_back(Action::RequestKeyFrame);
                            }
                        }
                        Ordering::Equal => {
                            // target is equal to current, handle if needed
                        }
                    }
                }
                (Some(current), None) => {
                    // need pause
                    // TODO: wait current frame finished for avoiding interrupt client
                    log::info!("[H264SimSelector] pause from {}", current);
                    self.current = None;
                }
                (None, Some(target)) => {
                    // need resume or start => need wait key-frame
                    if *key {
                        log::info!("[H264SimSelector] resume to {} with key", target);
                        // with other spatial we have difference tl0xidx and pic_id offset
                        // therefore we need reinit both tl0idx and pic_id
                        self.current = Some(target);
                    }
                }
                (None, None) => {
                    // reject
                }
            }
        }
    }

    fn is_allow(&mut self, _ctx: &mut VideoSelectorCtx, pkt: &mut MediaPacket) -> Option<()> {
        let current = self.current?;
        match &mut pkt.meta {
            MediaMeta::H264 { key: _, profile: _, sim: Some(sim) } => {
                if current == sim.spatial {
                    Some(())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl VideoSelector for Selector {
    fn on_init(&mut self, _ctx: &mut VideoSelectorCtx, _now_ms: u64) {}

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

    use super::VideoSelectorCtx;
    use super::{super::Action, Selector, VideoSelector};
    use media_server_protocol::media::{H264Profile, H264Sim, MediaKind, MediaLayerBitrate, MediaLayersBitrate, MediaMeta, MediaPacket};

    fn layers_bitrate(layers: &[u16]) -> MediaLayersBitrate {
        let mut res = MediaLayersBitrate::default();
        for (index, bitrate) in layers.iter().enumerate() {
            let mut spatial = MediaLayerBitrate::default();
            spatial.set_layer(0, *bitrate);
            res.set_layer(index, spatial);
        }
        res
    }

    fn video_pkt(seq: u16, ts: u32, key: bool, layers: Option<&[u16]>, spatial: u8) -> MediaPacket {
        MediaPacket {
            ts,
            seq,
            marker: true,
            nackable: false,
            layers: layers.map(layers_bitrate),
            meta: MediaMeta::H264 {
                key,
                profile: H264Profile::P42001fNonInterleaved,
                sim: Some(H264Sim { spatial }),
            },
            data: vec![1, 2, 3],
        }
    }

    enum Step {
        Bitrate(u64, u64, Vec<Action>),
        Pkt(u64, u64, MediaPacket, Option<(u16, u32)>, Vec<Action>),
    }

    fn test(bitrate: u64, layers: &[u16], steps: Vec<Step>) {
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
                        (seq, ts)
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

    #[test_log::test]
    fn up_spatial() {
        test(
            200,
            &[200, 800],
            vec![
                Step::Pkt(0, 0, video_pkt(0, 0, true, None, 0), Some((1, 0)), vec![]),
                Step::Pkt(0, 0, video_pkt(0, 0, true, None, 1), None, vec![]),
                Step::Pkt(0, 0, video_pkt(1, 0, true, None, 1), None, vec![]),
                Step::Pkt(30, 0, video_pkt(1, 2700, false, None, 0), Some((2, 2700)), vec![]),
                Step::Pkt(30, 0, video_pkt(2, 2700, false, None, 1), None, vec![]),
                Step::Pkt(30, 0, video_pkt(3, 2700, false, None, 1), None, vec![]),
                // now switch to higher layer need key_frame
                Step::Bitrate(30, 800, vec![Action::RequestKeyFrame]),
                Step::Pkt(60, 0, video_pkt(2, 5400, false, None, 0), Some((3, 5400)), vec![Action::RequestKeyFrame]),
                Step::Pkt(60, 0, video_pkt(4, 5400, false, None, 1), None, vec![Action::RequestKeyFrame]),
                Step::Pkt(60, 0, video_pkt(5, 5400, false, None, 1), None, vec![Action::RequestKeyFrame]),
                // now we have key-frame => switch
                Step::Pkt(90, 0, video_pkt(3, 8100, true, None, 0), None, vec![]),
                Step::Pkt(90, 0, video_pkt(6, 8100, true, None, 1), Some((4, 8100)), vec![]),
                Step::Pkt(90, 0, video_pkt(7, 8100, true, None, 1), Some((5, 8100)), vec![]),
            ],
        )
    }

    #[test_log::test]
    fn down_spatial() {
        test(
            800,
            &[200, 800],
            vec![
                Step::Pkt(0, 0, video_pkt(0, 0, true, None, 0), None, vec![]),
                Step::Pkt(0, 0, video_pkt(0, 0, true, None, 1), Some((1, 0)), vec![]),
                Step::Pkt(0, 0, video_pkt(1, 0, true, None, 1), Some((2, 0)), vec![]),
                Step::Pkt(30, 0, video_pkt(1, 2700, false, None, 0), None, vec![]),
                Step::Pkt(30, 0, video_pkt(2, 2700, false, None, 1), Some((3, 2700)), vec![]),
                Step::Pkt(30, 0, video_pkt(3, 2700, false, None, 1), Some((4, 2700)), vec![]),
                // now switch to higher layer need key_frame
                Step::Bitrate(30, 200, vec![Action::RequestKeyFrame]),
                Step::Pkt(60, 0, video_pkt(2, 5400, false, None, 0), None, vec![Action::RequestKeyFrame]),
                Step::Pkt(60, 0, video_pkt(4, 5400, false, None, 1), Some((5, 5400)), vec![Action::RequestKeyFrame]),
                Step::Pkt(60, 0, video_pkt(5, 5400, false, None, 1), Some((6, 5400)), vec![Action::RequestKeyFrame]),
                // now we have key-frame => switch
                Step::Pkt(90, 0, video_pkt(3, 8100, true, None, 0), Some((7, 8100)), vec![]),
                Step::Pkt(90, 0, video_pkt(6, 8100, true, None, 1), None, vec![]),
                Step::Pkt(90, 0, video_pkt(7, 8100, true, None, 1), None, vec![]),
            ],
        )
    }
}
