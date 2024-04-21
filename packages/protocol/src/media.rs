use derivative::Derivative;

#[derive(Clone)]
pub enum MediaKind {
    Audio,
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

#[derive(Clone)]
pub enum MediaScaling {
    None,
    Simulcat,
    Svc,
}

#[derive(Clone)]
pub enum MediaCodec {
    Opus,
    H264,
    Vp8,
    Vp9,
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct MediaPacket {
    pub pt: u8,
    pub ts: u32,
    pub seq: u64,
    pub marker: bool,
    pub nackable: bool,
    #[derivative(Debug = "ignore")]
    pub data: Vec<u8>,
}
