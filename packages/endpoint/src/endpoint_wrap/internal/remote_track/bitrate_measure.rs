use cluster::ClusterTrackStats;
use transport::PayloadCodec;

#[derive(Default)]
struct SingleStreamBitrateMeasure {
    sum: usize,
}

impl SingleStreamBitrateMeasure {
    pub fn add_sample(&mut self, payload_size: usize) {
        self.sum += payload_size;
    }

    pub fn take_bitrate_bps(&mut self, window_ms: u64) -> u32 {
        if self.sum == 0 {
            return 0;
        }
        let res = (self.sum as u32 * 8 * 1000) / window_ms as u32;
        self.sum = 0;
        res
    }
}

pub struct BitrateMeasure {
    last_measure_ms: u64,
    window_ms: u64,
    bitrate: SingleStreamBitrateMeasure,
    layers: [[SingleStreamBitrateMeasure; 3]; 3],
}

impl BitrateMeasure {
    pub fn new(window_ms: u64) -> Self {
        Self {
            last_measure_ms: 0,
            window_ms,
            bitrate: SingleStreamBitrateMeasure::default(),
            layers: Default::default(),
        }
    }

    fn layers_bitrate(&mut self, _now_ms: u64, svc: bool) -> [[u32; 3]; 3] {
        let mut layers = [[0; 3]; 3];
        for (i, layer) in self.layers.iter_mut().enumerate() {
            for (j, layer) in layer.iter_mut().enumerate() {
                layers[i][j] = layer.take_bitrate_bps(self.window_ms);
            }
        }

        for i in 0..3 {
            if layers[i][0] == 0 {
                break;
            }
            for j in 0..3 {
                if layers[i][j] == 0 {
                    break;
                }
                if j > 0 {
                    layers[i][j] += layers[i][j - 1];
                }
            }
        }

        // in svc mode we need to sum all smaller layer
        // [L1, L2, L3]
        // [M1, M2, M3]
        // [H1, H2, H3]
        // => [L1, L1 + L2, L1 + L2 + L3]
        // => [M1 + L1, M1 + M2, M1 + M2 + M3]
        // ...
        if svc {
            for i in 0..3 {
                if layers[i][0] == 0 {
                    break;
                }
                for j in 0..3 {
                    if layers[i][j] == 0 {
                        break;
                    }
                    if i > 0 {
                        layers[i][j] += layers[i - 1][j];
                    }
                }
            }
        }
        layers
    }

    pub fn add_sample(&mut self, now_ms: u64, codec: &PayloadCodec, payload_size: usize) -> Option<ClusterTrackStats> {
        if self.last_measure_ms == 0 {
            self.last_measure_ms = now_ms;
        }

        let res = if now_ms - self.last_measure_ms >= self.window_ms {
            let bitrate = self.bitrate.take_bitrate_bps(self.window_ms);
            self.last_measure_ms = now_ms;
            match codec {
                PayloadCodec::Vp8(_, Some(_)) => Some(ClusterTrackStats::Simulcast {
                    bitrate,
                    layers: self.layers_bitrate(now_ms, false),
                }),
                PayloadCodec::Vp9(_, _, Some(_)) => Some(ClusterTrackStats::Svc {
                    bitrate,
                    layers: self.layers_bitrate(now_ms, true),
                }),
                PayloadCodec::H264(_, _, Some(_)) => Some(ClusterTrackStats::Simulcast {
                    bitrate,
                    layers: self.layers_bitrate(now_ms, false),
                }),
                _ => Some(ClusterTrackStats::Single { bitrate }),
            }
        } else {
            None
        };

        self.bitrate.add_sample(payload_size);

        match codec {
            PayloadCodec::Vp8(_, Some(sim)) => {
                if sim.spatial < 3 && sim.temporal < 3 {
                    self.layers[sim.spatial as usize][sim.temporal as usize].add_sample(payload_size);
                }
            }
            PayloadCodec::Vp9(_, _, Some(svc)) => {
                if svc.spatial < 3 && svc.temporal < 3 {
                    self.layers[svc.spatial as usize][svc.temporal as usize].add_sample(payload_size);
                }
            }
            PayloadCodec::H264(_, _, Some(sim)) => {
                if sim.spatial < 3 {
                    self.layers[sim.spatial as usize][0].add_sample(payload_size);
                }
            }
            _ => {}
        };

        res
    }
}

#[cfg(test)]
mod test {
    use cluster::ClusterTrackStats;
    use transport::{Vp8Simulcast, Vp9Profile, Vp9Svc};

    use super::BitrateMeasure;

    #[test]
    fn single_stream() {
        let mut measure = super::SingleStreamBitrateMeasure::default();
        assert_eq!(measure.take_bitrate_bps(200), 0);

        measure.add_sample(1000);
        measure.add_sample(500);
        assert_eq!(measure.take_bitrate_bps(200), 1500 * 8 * 1000 / 200);
        assert_eq!(measure.take_bitrate_bps(200), 0);
    }

    #[test]
    fn video_single_stream() {
        let mut measure = BitrateMeasure::new(2000);

        assert_eq!(measure.add_sample(1000, &transport::PayloadCodec::Vp8(true, None), 1000), None);
        assert_eq!(measure.add_sample(1200, &transport::PayloadCodec::Vp8(true, None), 500), None);
        assert_eq!(
            measure.add_sample(3000, &transport::PayloadCodec::Vp8(true, None), 500),
            Some(ClusterTrackStats::Single { bitrate: 1500 * 8 / 2 })
        );
    }

    #[test]
    fn vp8_simulcast_stream() {
        let mut measure = BitrateMeasure::new(2000);

        assert_eq!(measure.add_sample(1000, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(0, 0, false))), 100), None);
        assert_eq!(measure.add_sample(1000, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(1, 0, false))), 500), None);
        assert_eq!(measure.add_sample(1000, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(2, 0, false))), 1000), None);

        assert_eq!(measure.add_sample(1500, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(0, 1, false))), 50), None);
        assert_eq!(measure.add_sample(1500, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(1, 1, false))), 100), None);
        assert_eq!(measure.add_sample(1500, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(2, 1, false))), 500), None);

        assert_eq!(measure.add_sample(1500, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(0, 2, false))), 200), None);
        assert_eq!(measure.add_sample(1500, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(1, 2, false))), 400), None);
        assert_eq!(measure.add_sample(1500, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(2, 2, false))), 800), None);

        assert_eq!(
            measure.add_sample(3000, &transport::PayloadCodec::Vp8(true, Some(Vp8Simulcast::new(0, 0, false))), 500),
            Some(ClusterTrackStats::Simulcast {
                bitrate: (100 + 500 + 1000 + 50 + 100 + 500 + 200 + 400 + 800) * 8 / 2,
                layers: [
                    [100 * 8 / 2, (100 + 50) * 8 / 2, (100 + 50 + 200) * 8 / 2],
                    [500 * 8 / 2, (500 + 100) * 8 / 2, (500 + 100 + 400) * 8 / 2],
                    [1000 * 8 / 2, (1000 + 500) * 8 / 2, (1000 + 500 + 800) * 8 / 2],
                ]
            })
        );
    }

    #[test]
    fn vp9_svc_stream() {
        let mut measure = BitrateMeasure::new(2000);

        assert_eq!(
            measure.add_sample(1000, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(0, 0, false, false))), 100),
            None
        );
        assert_eq!(
            measure.add_sample(1000, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(1, 0, false, false))), 500),
            None
        );
        assert_eq!(
            measure.add_sample(1000, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(2, 0, false, false))), 1000),
            None
        );
        assert_eq!(
            measure.add_sample(1500, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(0, 1, false, false))), 50),
            None
        );
        assert_eq!(
            measure.add_sample(1500, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(1, 1, false, false))), 100),
            None
        );
        assert_eq!(
            measure.add_sample(1500, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(2, 1, false, false))), 500),
            None
        );
        assert_eq!(
            measure.add_sample(1500, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(0, 2, false, false))), 200),
            None
        );
        assert_eq!(
            measure.add_sample(1500, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(1, 2, false, false))), 400),
            None
        );
        assert_eq!(
            measure.add_sample(1500, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(2, 2, false, false))), 800),
            None
        );

        assert_eq!(
            measure.add_sample(3000, &transport::PayloadCodec::Vp9(true, Vp9Profile::P0, Some(Vp9Svc::new(0, 0, false, false))), 500),
            Some(ClusterTrackStats::Svc {
                bitrate: (100 + 500 + 1000 + 50 + 100 + 500 + 200 + 400 + 800) * 8 / 2,
                layers: [
                    [100 * 8 / 2, (100 + 50) * 8 / 2, (100 + 50 + 200) * 8 / 2],
                    [(100 + 500) * 8 / 2, (100 + 50 + 500 + 100) * 8 / 2, (100 + 50 + 200 + 500 + 100 + 400) * 8 / 2],
                    [
                        (100 + 500 + 1000) * 8 / 2,
                        (100 + 50 + 500 + 100 + 1000 + 500) * 8 / 2,
                        (100 + 50 + 200 + 500 + 100 + 400 + 1000 + 500 + 800) * 8 / 2
                    ],
                ]
            })
        );
    }
}
