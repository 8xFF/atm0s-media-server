use media_server_core::transport::RemoteTrackId;
use media_server_protocol::{
    endpoint::{BitrateControlMode, TrackMeta, TrackName, TrackPriority},
    media::{MediaKind, MediaScaling},
    protobuf,
};
use str0m::media::Mid;

const MIN_STREAM_BITRATE_BPS: u64 = 100_000; //min 100kbps

pub struct RemoteTrack {
    id: RemoteTrackId,
    name: TrackName,
    kind: MediaKind,
    priority: TrackPriority,
    control: Option<BitrateControlMode>,
    scaling: MediaScaling,
    mid: Option<Mid>,
}

impl RemoteTrack {
    pub fn new(id: RemoteTrackId, config: protobuf::shared::Sender) -> Self {
        Self {
            id,
            name: config.name.clone().into(),
            kind: config.kind().into(),
            priority: config.state.priority.into(),
            control: config.bitrate.map(|b| protobuf::shared::BitrateControlMode::try_from(b).ok().expect("Should have").into()),
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
        self.priority
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn mid(&self) -> Option<Mid> {
        self.mid
    }

    pub fn set_str0m(&mut self, mid: Mid, sim: bool) {
        assert_eq!(self.mid, None, "LocalTrack mid {:?} already configed", self.mid);
        self.mid = Some(mid);
        if sim {
            self.scaling = MediaScaling::Simulcast;
        }
    }

    pub fn meta(&self) -> TrackMeta {
        TrackMeta {
            kind: self.kind(),
            scaling: self.scaling,
            control: self.control,
        }
    }

    pub fn calc_limit_bitrate(&self, min: u64, max: u64) -> u64 {
        match self.scaling {
            MediaScaling::None => min.max(MIN_STREAM_BITRATE_BPS),
            MediaScaling::Simulcast => max,
        }
    }
}
