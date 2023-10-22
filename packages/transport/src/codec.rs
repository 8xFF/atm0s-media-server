#[derive(Debug, Clone, Eq, PartialEq)]
pub struct H264Simulcast {
    pub sparital: u8,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Vp8Simulcast {
    pub picture_id: Option<u16>,
    pub spatial: u8,
    pub temporal: u8,
    pub layer_sync: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Vp9Svc {
    pub spatial: u8,
    pub temporal: u8,
    pub begin_frame: bool,
    pub end_frame: bool,
    pub picture_id: Option<u16>,
    pub switching_point: bool,
    pub predicted_frame: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum H264Profile {
    P42001fNonInterleaved,
    P42001fSingleNal,
    P42e01fNonInterleaved,
    P42e01fSingleNal,
    P4d001fNonInterleaved,
    P4d001fSingleNal,
    P64001fNonInterleaved,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Vp9Profile {
    P0,
    P2,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PayloadCodec {
    Vp8(bool, Option<Vp8Simulcast>),
    Vp9(bool, Vp9Profile, Option<Vp9Svc>),
    H264(bool, H264Profile, Option<H264Simulcast>),
    Opus,
}

impl PayloadCodec {
    pub fn is_key(&self) -> bool {
        match self {
            PayloadCodec::Vp8(is_key, _) => *is_key,
            PayloadCodec::Vp9(is_key, _, _) => *is_key,
            PayloadCodec::H264(is_key, _, _) => *is_key,
            PayloadCodec::Opus => true,
        }
    }
}
