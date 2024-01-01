use str0m::media::MediaKind;
use transport::TrackId;

#[derive(Default)]
pub struct LocalTrackIdGenerator {
    audio_tracks: Vec<TrackId>,
    video_tracks: Vec<TrackId>,
}

impl LocalTrackIdGenerator {
    /// generate track id for local track with format kind_index
    /// index is the index of the track in the list of tracks of the same kind
    pub fn generate(&mut self, kind: MediaKind, track_id: TrackId) -> String {
        match kind {
            MediaKind::Audio => {
                let index = self.audio_tracks.iter().position(|m| *m == track_id).unwrap_or_else(|| {
                    let index = self.audio_tracks.len();
                    self.audio_tracks.push(track_id);
                    index
                });
                format!("audio_{}", index)
            }
            MediaKind::Video => {
                let index = self.video_tracks.iter().position(|m| *m == track_id).unwrap_or_else(|| {
                    let index = self.video_tracks.len();
                    self.video_tracks.push(track_id);
                    index
                });
                format!("video_{}", index)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn in_order() {
        let mut generator = super::LocalTrackIdGenerator::default();
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 100), "audio_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 200), "audio_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 300), "audio_2");

        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 100), "video_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 200), "video_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 300), "video_2");
    }

    #[test]
    fn in_order2() {
        let mut generator = super::LocalTrackIdGenerator::default();
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 100), "audio_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 100), "video_0");

        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 200), "audio_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 300), "audio_2");

        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 200), "video_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 300), "video_2");
    }

    #[test]
    fn reuse() {
        let mut generator = super::LocalTrackIdGenerator::default();
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 100), "audio_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 100), "video_0");

        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 200), "audio_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, 100), "audio_0");

        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 200), "video_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, 100), "video_0");
    }
}
