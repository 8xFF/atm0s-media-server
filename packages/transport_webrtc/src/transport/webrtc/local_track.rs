use media_server_core::transport::LocalTrackId;
use media_server_protocol::{endpoint::TrackName, media::MediaKind, protobuf};
use str0m::media::Mid;

pub struct LocalTrack {
    id: LocalTrackId,
    name: TrackName,
    kind: MediaKind,
    mid: Option<Mid>,
}

impl LocalTrack {
    pub fn new(id: LocalTrackId, cfg: protobuf::shared::Receiver) -> Self {
        log::info!("[TransportWebrcSdk/LocalTrack] create {id} config {:?}", cfg);
        Self {
            id,
            name: cfg.name.clone().into(),
            kind: cfg.kind().into(),
            mid: None,
        }
    }

    pub fn id(&self) -> LocalTrackId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name.as_ref()
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn mid(&self) -> Option<Mid> {
        self.mid
    }

    pub fn set_mid(&mut self, mid: Mid) {
        log::info!("[TransportWebrcSdk/LocalTrack] set_mid {}/{} => {}", self.id, self.name, mid);
        assert_eq!(self.mid, None, "LocalTrack mid {:?} already configed", self.mid);
        self.mid = Some(mid);
    }
}
