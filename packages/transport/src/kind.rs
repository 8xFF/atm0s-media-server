use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum MediaKind {
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
}

impl MediaKind {
    pub fn is_audio(&self) -> bool {
        matches!(self, MediaKind::Audio)
    }

    pub fn is_video(&self) -> bool {
        matches!(self, MediaKind::Video)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_audio() {
        let audio = MediaKind::Audio;
        assert!(audio.is_audio());
        assert!(!audio.is_video());
    }

    #[test]
    fn test_is_video() {
        let video = MediaKind::Video;
        assert!(!video.is_audio());
        assert!(video.is_video());
    }
}
