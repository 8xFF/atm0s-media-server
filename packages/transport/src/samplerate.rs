#[derive(PartialEq, Eq, Debug, Clone)]
pub enum MediaSampleRate {
    Hz48000, //For video
    Hz90000, //For video
    HzCustom(u32),
}

impl From<u32> for MediaSampleRate {
    fn from(value: u32) -> Self {
        match value {
            48000 => MediaSampleRate::Hz48000,
            90000 => MediaSampleRate::Hz90000,
            _ => MediaSampleRate::HzCustom(value),
        }
    }
}

impl From<MediaSampleRate> for u32 {
    fn from(value: MediaSampleRate) -> Self {
        match value {
            MediaSampleRate::Hz48000 => 48000,
            MediaSampleRate::Hz90000 => 90000,
            MediaSampleRate::HzCustom(value) => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_u32() {
        assert_eq!(MediaSampleRate::from(48000), MediaSampleRate::Hz48000);
        assert_eq!(MediaSampleRate::from(90000), MediaSampleRate::Hz90000);
        assert_eq!(MediaSampleRate::from(44100), MediaSampleRate::HzCustom(44100));
    }

    #[test]
    fn test_into_u32() {
        assert_eq!(u32::from(MediaSampleRate::Hz48000), 48000);
        assert_eq!(u32::from(MediaSampleRate::Hz90000), 90000);
        assert_eq!(u32::from(MediaSampleRate::HzCustom(44100)), 44100);
    }
}
