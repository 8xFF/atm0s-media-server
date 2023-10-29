use str0m::media::MediaKind;

pub fn to_transport_kind(value: MediaKind) -> transport::MediaKind {
    match value {
        MediaKind::Audio => transport::MediaKind::Audio,
        MediaKind::Video => transport::MediaKind::Video,
    }
}
