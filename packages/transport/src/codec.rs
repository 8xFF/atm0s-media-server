use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct H264Simulcast {
    pub spatial: u8,
}

impl Display for H264Simulcast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(s:{})", self.spatial)
    }
}

impl H264Simulcast {
    pub fn new(spatial: u8) -> Self {
        Self { spatial }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Vp8Simulcast {
    pub picture_id: Option<u16>,
    pub tl0_pic_idx: Option<u8>,
    pub spatial: u8,
    pub temporal: u8,
    pub layer_sync: bool,
}

impl Display for Vp8Simulcast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(pid:{:?},tl0:{:?},s:{},t:{},ls:{})",
            self.picture_id, self.tl0_pic_idx, self.spatial, self.temporal, self.layer_sync
        )
    }
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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

impl Display for Vp9Svc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(s:{},t:{},bf:{},ef:{},sl:{:?},pid:{:?},sp:{},pf:{})",
            self.spatial, self.temporal, self.begin_frame, self.end_frame, self.spatial_layers, self.picture_id, self.switching_point, self.predicted_frame
        )
    }
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum H264Profile {
    P42001fNonInterleaved,
    P42001fSingleNal,
    P42e01fNonInterleaved,
    P42e01fSingleNal,
    P4d001fNonInterleaved,
    P4d001fSingleNal,
    P64001fNonInterleaved,
}

impl Display for H264Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            H264Profile::P42001fNonInterleaved => write!(f, "P42001fNonInterleaved"),
            H264Profile::P42001fSingleNal => write!(f, "P42001fSingleNal"),
            H264Profile::P42e01fNonInterleaved => write!(f, "P42e01fNonInterleaved"),
            H264Profile::P42e01fSingleNal => write!(f, "P42e01fSingleNal"),
            H264Profile::P4d001fNonInterleaved => write!(f, "P4d001fNonInterleaved"),
            H264Profile::P4d001fSingleNal => write!(f, "P4d001fSingleNal"),
            H264Profile::P64001fNonInterleaved => write!(f, "P64001fNonInterleaved"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Vp9Profile {
    P0,
    P2,
}

impl Display for Vp9Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vp9Profile::P0 => write!(f, "P0"),
            Vp9Profile::P2 => write!(f, "P2"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum PayloadCodec {
    Vp8(bool, Option<Vp8Simulcast>),
    Vp9(bool, Vp9Profile, Option<Vp9Svc>),
    H264(bool, H264Profile, Option<H264Simulcast>),
    Opus,
}

impl Default for PayloadCodec {
    fn default() -> Self {
        PayloadCodec::Opus
    }
}

impl Display for PayloadCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayloadCodec::Vp8(is_key, Some(meta)) => write!(f, "VP8({},{})", is_key, meta),
            PayloadCodec::Vp8(is_key, None) => write!(f, "VP8({})", is_key),
            PayloadCodec::Vp9(is_key, profile, Some(meta)) => write!(f, "VP9({is_key},{profile},{meta})"),
            PayloadCodec::Vp9(is_key, profile, None) => write!(f, "VP9({is_key},{profile})"),
            PayloadCodec::H264(is_key, profile, Some(meta)) => write!(f, "H264({is_key},{profile},{meta})"),
            PayloadCodec::H264(is_key, profile, None) => write!(f, "H264({is_key},{profile})"),
            PayloadCodec::Opus => write!(f, "OPUS"),
        }
    }
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

    pub fn is_video(&self) -> bool {
        match self {
            PayloadCodec::Opus => false,
            _ => true,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_codec_display() {
        let vp8 = PayloadCodec::Vp8(true, None);
        assert_eq!(format!("{}", vp8), "VP8(true)");

        let vp9 = PayloadCodec::Vp9(false, Vp9Profile::P0, None);
        assert_eq!(format!("{}", vp9), "VP9(false,P0)");

        let h264 = PayloadCodec::H264(true, H264Profile::P42001fNonInterleaved, None);
        assert_eq!(format!("{}", h264), "H264(true,P42001fNonInterleaved)");

        let opus = PayloadCodec::Opus;
        assert_eq!(format!("{}", opus), "OPUS");
    }

    #[test]
    fn test_payload_codec_is_key() {
        let vp8 = PayloadCodec::Vp8(true, None);
        assert_eq!(vp8.is_key(), true);

        let vp9 = PayloadCodec::Vp9(false, Vp9Profile::P0, None);
        assert_eq!(vp9.is_key(), false);

        let h264 = PayloadCodec::H264(true, H264Profile::P42001fNonInterleaved, None);
        assert_eq!(h264.is_key(), true);

        let opus = PayloadCodec::Opus;
        assert_eq!(opus.is_key(), true);
    }

    #[test]
    fn test_payload_codec_is_audio() {
        let vp8 = PayloadCodec::Vp8(true, None);
        assert_eq!(vp8.is_audio(), false);

        let vp9 = PayloadCodec::Vp9(false, Vp9Profile::P0, None);
        assert_eq!(vp9.is_audio(), false);

        let h264 = PayloadCodec::H264(true, H264Profile::P42001fNonInterleaved, None);
        assert_eq!(h264.is_audio(), false);

        let opus = PayloadCodec::Opus;
        assert_eq!(opus.is_audio(), true);
    }

    #[test]
    fn test_payload_codec_is_video() {
        let vp8 = PayloadCodec::Vp8(true, None);
        assert_eq!(vp8.is_video(), true);

        let vp9 = PayloadCodec::Vp9(false, Vp9Profile::P0, None);
        assert_eq!(vp9.is_video(), true);

        let h264 = PayloadCodec::H264(true, H264Profile::P42001fNonInterleaved, None);
        assert_eq!(h264.is_video(), true);

        let opus = PayloadCodec::Opus;
        assert_eq!(opus.is_video(), false);
    }
}
