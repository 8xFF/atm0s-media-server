use std::collections::{HashMap, VecDeque};

use str0m::media::MediaKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsidInfo {
    pub label: String,
    pub kind: String,
    pub name: String,
}

#[derive(Default)]
pub struct TrackInfoQueue {
    audios: VecDeque<MsidInfo>,
    videos: VecDeque<MsidInfo>,
}

impl TrackInfoQueue {
    pub fn add(&mut self, uuid: &str, label: &str, kind: &str, name: &str) {
        match kind {
            "audio" | "Audio" | "AUDIO" => self.audios.push_back(MsidInfo {
                label: label.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
            }),
            "video" | "Video" | "VIDEO" => self.videos.push_back(MsidInfo {
                label: label.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
            }),
            _ => {}
        }
    }

    pub fn pop(&mut self, kind: MediaKind) -> Option<MsidInfo> {
        match kind {
            MediaKind::Audio => self.audios.pop_front(),
            MediaKind::Video => self.videos.pop_front(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::transport::internal::track_info_queue::MsidInfo;

    #[test]
    fn should_work() {
        let mut queue = super::TrackInfoQueue::default();
        queue.add("audio_uuid", "audio_label", "audio", "name");
        queue.add("video_uuid", "video_label", "video", "name");
        assert_eq!(
            queue.pop(str0m::media::MediaKind::Audio),
            Some(MsidInfo {
                label: "audio_label".to_string(),
                kind: "kind".to_string(),
                name: "name".to_string(),
            })
        );
        assert_eq!(
            queue.pop(str0m::media::MediaKind::Video),
            Some(MsidInfo {
                label: "video_label".to_string(),
                kind: "kind".to_string(),
                name: "name".to_string(),
            })
        );
    }
}
