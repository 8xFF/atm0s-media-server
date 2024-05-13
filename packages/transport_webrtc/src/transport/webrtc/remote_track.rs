use media_server_core::transport::RemoteTrackId;
use media_server_protocol::{
    endpoint::{TrackMeta, TrackName, TrackPriority},
    media::{MediaKind, MediaScaling},
    protobuf,
};
use str0m::media::Mid;

const MIN_STREAM_BITRATE_BPS: u64 = 100_000; //min 100kbps

pub struct RemoteTrack {
    id: RemoteTrackId,
    name: TrackName,
    kind: MediaKind,
    source: Option<protobuf::shared::sender::Source>,
    config: protobuf::shared::sender::Config,
    scaling: MediaScaling,
    mid: Option<Mid>,
}

impl RemoteTrack {
    pub fn new(id: RemoteTrackId, cfg: protobuf::shared::Sender) -> Self {
        log::info!("[TransportWebrcSdk/RemoteTrack] create {id} config {:?}", cfg);
        let kind = cfg.kind();
        let state = cfg.state.unwrap_or_default();
        Self {
            id,
            name: cfg.name.clone().into(),
            kind: kind.into(),
            source: state.source,
            config: state.config.unwrap_or_default(),
            scaling: MediaScaling::None,
            mid: None,
        }
    }

    pub fn id(&self) -> RemoteTrackId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name.as_ref()
    }

    pub fn priority(&self) -> TrackPriority {
        self.config.priority.into()
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn mid(&self) -> Option<Mid> {
        self.mid
    }

    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }

    pub fn set_source(&mut self, source: protobuf::shared::sender::Source) {
        self.source = Some(source);
    }

    pub fn del_source(&mut self) {
        self.source = None;
    }

    pub fn set_str0m(&mut self, mid: Mid, sim: bool) {
        log::info!("[TransportWebrcSdk/RemoteTrack] set_mid {}/{} => {}, simulcast {}", self.id, self.name, mid, sim);
        assert_eq!(self.mid, None, "LocalTrack mid {:?} already configured", self.mid);
        self.mid = Some(mid);
        if sim {
            self.scaling = MediaScaling::Simulcast;
        }
    }

    pub fn meta(&self) -> TrackMeta {
        TrackMeta {
            kind: self.kind(),
            scaling: self.scaling,
            control: self.config.bitrate().into(),
            metadata: self.source.as_ref().map(|s| s.metadata.clone()).flatten(),
        }
    }

    pub fn calc_limit_bitrate(&self, min: u64, max: u64) -> u64 {
        match self.scaling {
            MediaScaling::None => min.max(MIN_STREAM_BITRATE_BPS),
            MediaScaling::Simulcast => max,
        }
    }
}
