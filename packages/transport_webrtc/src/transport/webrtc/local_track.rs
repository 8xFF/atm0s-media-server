use media_server_core::transport::LocalTrackId;
use media_server_protocol::{
    endpoint::TrackName,
    media::MediaKind,
    protobuf::{self, shared::Kind},
};
use str0m::media::Mid;

pub struct LocalTrack {
    id: LocalTrackId,
    name: TrackName,
    kind: MediaKind,
    mid: Option<Mid>,
}

impl LocalTrack {
    pub fn new(id: LocalTrackId, config: protobuf::shared::Receiver) -> Self {
        Self {
            id,
            name: config.name.clone().into(),
            kind: config.kind().into(),
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
        assert_eq!(self.mid, None, "LocalTrack mid {:?} already configed", self.mid);
        self.mid = Some(mid);
    }
}
