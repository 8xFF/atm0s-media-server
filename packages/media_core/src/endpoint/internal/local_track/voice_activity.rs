const AUDIO_LEVEL_THRESHOLD: i8 = -40;
const VOICE_ACTIVITY_INTERVAL: u64 = 500;

#[derive(Default)]
pub struct VoiceActivityDetector {
    last_activity: u64,
}

impl VoiceActivityDetector {
    pub fn on_audio(&mut self, now: u64, audio_level: Option<i8>) -> Option<i8> {
        let audio_level = audio_level?;
        if audio_level >= AUDIO_LEVEL_THRESHOLD && self.last_activity + VOICE_ACTIVITY_INTERVAL <= now {
            self.last_activity = now;
            Some(audio_level)
        } else {
            None
        }
    }
}
