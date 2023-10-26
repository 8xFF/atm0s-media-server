//! SourceStatsGenerator generate fake ClusterTrackStats for new source for faster switch
//! It only generate stats for first packet rtp if event if it dont received any stats before
//! In case of simulcast it generate stats for 3 layers with bitrate 50k, 100k, 150k
//! In case of svc it generate stats for 3 layers with bitrate 50k, 100k, 150k
//! In case of single it generate stats with bitrate 100k

use cluster::ClusterTrackStats;
use transport::{MediaPacket, PayloadCodec};

#[derive(Default)]
pub struct SourceStatsGenerator {
    has_stats: bool,
}

impl SourceStatsGenerator {
    /// Mark that stats received
    pub fn arrived_stats(&mut self) {
        self.has_stats = true;
    }

    /// Mark that source switched, so we need mark that stats not received
    pub fn switched_source(&mut self) {
        self.has_stats = false;
    }

    /// Generate fake stats for new source if it dont received any stats before
    pub fn on_pkt(&mut self, pkt: &MediaPacket) -> Option<ClusterTrackStats> {
        //TODO estimate bitrate from pkt payload length
        if !self.has_stats {
            self.has_stats = true;
            match &pkt.codec {
                PayloadCodec::Vp8(_, Some(_)) => Some(ClusterTrackStats::Simulcast {
                    bitrate: 100000,
                    layers: [[50_000, 100_000, 150_000], [0, 0, 0], [0, 0, 0]],
                }),
                PayloadCodec::Vp9(_, _, Some(_)) => Some(ClusterTrackStats::Svc {
                    bitrate: 100000,
                    layers: [[50_000, 100_000, 150_000], [0, 0, 0], [0, 0, 0]],
                }),
                PayloadCodec::H264(_, _, Some(_)) => Some(ClusterTrackStats::Simulcast {
                    bitrate: 100000,
                    layers: [[50_000, 100_000, 150_000], [0, 0, 0], [0, 0, 0]],
                }),
                PayloadCodec::Vp8(_, None) => Some(ClusterTrackStats::Single { bitrate: 100000 }),
                PayloadCodec::Vp9(_, _, None) => Some(ClusterTrackStats::Single { bitrate: 100000 }),
                PayloadCodec::H264(_, _, None) => Some(ClusterTrackStats::Single { bitrate: 100000 }),
                PayloadCodec::Opus => None,
            }
        } else {
            None
        }
    }
}
