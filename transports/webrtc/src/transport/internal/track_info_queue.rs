use std::collections::{HashMap, VecDeque};

use str0m::media::MediaKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsidInfo {
    pub label: String,
    pub kind: String,
    pub name: String,
    pub uuid: String,
}

#[derive(Default)]
pub struct TrackInfoQueue {
    pushed_track: HashMap<String, String>,
    audios: VecDeque<MsidInfo>,
    videos: VecDeque<MsidInfo>,
}

impl TrackInfoQueue {
    pub fn add(&mut self, uuid: &str, label: &str, kind: &str, name: &str) {
        if let Some(old_uuid) = self.pushed_track.get(name) {
            if old_uuid.eq(uuid) {
                return;
            }
        }
        self.pushed_track.insert(name.to_string(), uuid.to_string());

        match kind {
            "audio" | "Audio" | "AUDIO" => self.audios.push_back(MsidInfo {
                label: label.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
                uuid: uuid.to_string(),
            }),
            "video" | "Video" | "VIDEO" => self.videos.push_back(MsidInfo {
                label: label.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
                uuid: uuid.to_string(),
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
        queue.add("audio_uuid", "audio_label", "audio", "audio_main");
        queue.add("video_uuid", "video_label", "video", "video_main");
        assert_eq!(
            queue.pop(str0m::media::MediaKind::Audio),
            Some(MsidInfo {
                label: "audio_label".to_string(),
                kind: "audio".to_string(),
                name: "audio_main".to_string(),
                uuid: "audio_uuid".to_string(),
            })
        );
        assert_eq!(
            queue.pop(str0m::media::MediaKind::Video),
            Some(MsidInfo {
                label: "video_label".to_string(),
                kind: "video".to_string(),
                name: "video_main".to_string(),
                uuid: "video_uuid".to_string(),
            })
        );
    }

    #[test]
    fn reject_duplicate() {
        let mut queue = super::TrackInfoQueue::default();
        queue.add("audio_uuid", "audio_label", "audio", "name");
        assert_eq!(
            queue.pop(str0m::media::MediaKind::Audio),
            Some(MsidInfo {
                label: "audio_label".to_string(),
                kind: "audio".to_string(),
                name: "name".to_string(),
                uuid: "audio_uuid".to_string(),
            })
        );

        queue.add("audio_uuid", "audio_label", "audio", "name");
        assert_eq!(queue.pop(str0m::media::MediaKind::Audio), None);
    }

    #[test]
    fn rewrite_new_uuid() {
        let mut queue = super::TrackInfoQueue::default();
        queue.add("audio_uuid1", "audio_label", "audio", "name");
        assert_eq!(
            queue.pop(str0m::media::MediaKind::Audio),
            Some(MsidInfo {
                label: "audio_label".to_string(),
                kind: "audio".to_string(),
                name: "name".to_string(),
                uuid: "audio_uuid1".to_string(),
            })
        );

        queue.add("audio_uuid2", "audio_label", "audio", "name");
        assert_eq!(
            queue.pop(str0m::media::MediaKind::Audio),
            Some(MsidInfo {
                label: "audio_label".to_string(),
                kind: "audio".to_string(),
                name: "name".to_string(),
                uuid: "audio_uuid2".to_string(),
            })
        );
    }
}
