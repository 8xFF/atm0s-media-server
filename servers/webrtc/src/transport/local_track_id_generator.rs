use str0m::media::{MediaKind, Mid};

#[derive(Default)]
pub struct LocalTrackIdGenerator {
    audio_mids: Vec<Mid>,
    video_mids: Vec<Mid>,
}

impl LocalTrackIdGenerator {
    /// generate track id for local track with format kind_index
    /// index is the index of the track in the list of tracks of the same kind
    pub fn generate(&mut self, kind: MediaKind, mid: Mid) -> String {
        match kind {
            MediaKind::Audio => {
                let index = self.audio_mids.iter().position(|m| *m == mid).unwrap_or_else(|| {
                    let index = self.audio_mids.len();
                    self.audio_mids.push(mid);
                    index
                });
                format!("audio_{}", index)
            }
            MediaKind::Video => {
                let index = self.video_mids.iter().position(|m| *m == mid).unwrap_or_else(|| {
                    let index = self.video_mids.len();
                    self.video_mids.push(mid);
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
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "a".into()), "audio_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "b".into()), "audio_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "c".into()), "audio_2");

        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "a".into()), "video_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "b".into()), "video_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "c".into()), "video_2");
    }

    #[test]
    fn in_order2() {
        let mut generator = super::LocalTrackIdGenerator::default();
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "a".into()), "audio_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "a".into()), "video_0");

        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "b".into()), "audio_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "c".into()), "audio_2");

        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "b".into()), "video_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "c".into()), "video_2");
    }

    #[test]
    fn reuse() {
        let mut generator = super::LocalTrackIdGenerator::default();
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "a".into()), "audio_0");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "a".into()), "video_0");

        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "b".into()), "audio_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Audio, "a".into()), "audio_0");

        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "b".into()), "video_1");
        assert_eq!(generator.generate(str0m::media::MediaKind::Video, "a".into()), "video_0");
    }
}
