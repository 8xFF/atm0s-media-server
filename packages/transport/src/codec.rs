#[derive(Debug, Clone, Eq, PartialEq)]
pub struct H264Simulcast {
    pub spatial: u8,
}

impl H264Simulcast {
    pub fn new(spatial: u8) -> Self {
        Self { spatial }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Vp8Simulcast {
    pub picture_id: Option<u16>,
    pub tl0_pic_idx: Option<u8>,
    pub spatial: u8,
    pub temporal: u8,
    pub layer_sync: bool,
}

impl Vp8Simulcast {
    pub fn new(spatial: u8, temporal: u8, layer_sync: bool) -> Self {
        Self {
            picture_id: None,
            tl0_pic_idx: None,
            spatial,
            temporal,
            layer_sync,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Vp9Svc {
    pub spatial: u8,
    pub temporal: u8,
    pub begin_frame: bool,
    pub end_frame: bool,
    pub spatial_layers: Option<u8>,
    pub picture_id: Option<u16>,
    pub switching_point: bool,
    pub predicted_frame: bool,
}

impl Vp9Svc {
    pub fn new(spatial: u8, temporal: u8, end_frame: bool, switching_point: bool) -> Self {
        Self {
            spatial,
            temporal,
            begin_frame: false,
            end_frame,
            spatial_layers: None,
            picture_id: None,
            switching_point,
            predicted_frame: false,
        }
    }
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

    pub fn is_audio(&self) -> bool {
        match self {
            PayloadCodec::Opus => true,
            _ => false,
        }
    }
}
