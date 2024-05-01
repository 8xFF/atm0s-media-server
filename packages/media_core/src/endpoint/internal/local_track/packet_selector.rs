use media_server_protocol::media::{MediaKind, MediaPacket};
use media_server_utils::{SeqRewrite, TsRewrite};

const SEQ_MAX: u64 = 1 << 16;
const TS_MAX: u64 = 1 << 32;

pub type MediaSeqRewrite = SeqRewrite<SEQ_MAX, 1000>;
pub type MediaTsRewrite = TsRewrite<TS_MAX, 10>;

pub enum Action {
    RequestKeyFrame,
}

pub struct PacketSelector {
    kind: MediaKind,
    ts_rewrite: MediaTsRewrite,
    seq_rewrite: MediaSeqRewrite,
    selected_channel: Option<u64>,
}

impl PacketSelector {
    pub fn new(kind: MediaKind) -> Self {
        Self {
            kind,
            ts_rewrite: MediaTsRewrite::new(kind.sample_rate()),
            seq_rewrite: MediaSeqRewrite::default(),
            selected_channel: None,
        }
    }

    /// Reset, call reset if local_track changed source
    pub fn reset(&mut self) {
        log::info!("[LocalTrack/PacketSelector] reset");
        self.selected_channel = None;
    }

    /// Select and rewrite if need. If select will return Some<()>
    pub fn select(&mut self, now_ms: u64, channel: u64, pkt: &mut MediaPacket) -> Option<Option<Action>> {
        let mut need_key = false;
        if self.selected_channel != Some(channel) {
            log::info!("[LocalTrack/PacketSelector] source changed => reinit ts_rewrite and seq_rewrite");
            self.ts_rewrite.reinit();
            self.seq_rewrite.reinit();
            need_key = self.kind.is_video();
            self.selected_channel = Some(channel);
        }

        pkt.ts = self.ts_rewrite.generate(now_ms, pkt.ts as u64) as u32;
        pkt.seq = self.seq_rewrite.generate(pkt.seq as u64)? as u16;

        if need_key {
            log::info!("[LocalTrack/PacketSelector] require key-frame");
            Some(Some(Action::RequestKeyFrame))
        } else {
            Some(None)
        }
    }
}
