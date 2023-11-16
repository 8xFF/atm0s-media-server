use std::ops::Deref;

use str0m::format::CodecConfig;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LocalPayloadType {
    Unknown,
    Opus,
    Vp8,
    Vp9P0,
    Vp9P2,
    H264_42001fNon,
    H264_42001fSingle,
    H264_42e01fNon,
    H264_42e01fSingle,
    H264_4d001fNon,
    H264_4d001fSingle,
    H264_64001fNon,
}

impl LocalPayloadType {
    pub fn to_u8(&self) -> u8 {
        match self {
            LocalPayloadType::Unknown => 0,
            LocalPayloadType::Opus => 1,
            LocalPayloadType::Vp8 => 2,
            LocalPayloadType::Vp9P0 => 3,
            LocalPayloadType::Vp9P2 => 4,
            LocalPayloadType::H264_42001fNon => 5,
            LocalPayloadType::H264_42001fSingle => 6,
            LocalPayloadType::H264_42e01fNon => 7,
            LocalPayloadType::H264_42e01fSingle => 8,
            LocalPayloadType::H264_4d001fNon => 9,
            LocalPayloadType::H264_4d001fSingle => 10,
            LocalPayloadType::H264_64001fNon => 11,
        }
    }
}

pub struct PtMapping {
    remote_to_local_map: [LocalPayloadType; 256],
    local_to_remote_map: [u8; 256],
}

impl Default for PtMapping {
    fn default() -> Self {
        Self {
            remote_to_local_map: [LocalPayloadType::Unknown; 256],
            local_to_remote_map: [0; 256],
        }
    }
}

impl PtMapping {
    pub fn str0m_sync_codec_config(&mut self, config: &CodecConfig) {
        for param in config.params() {
            let pt = *(param.pt().deref());
            let spec = param.spec();
            let local_pt = match spec.codec {
                str0m::format::Codec::Opus => LocalPayloadType::Opus,
                str0m::format::Codec::H264 => match (spec.format.profile_level_id, spec.format.packetization_mode) {
                    (Some(0x42001f), Some(1)) => LocalPayloadType::H264_42001fNon,
                    (Some(0x42001f), Some(0)) => LocalPayloadType::H264_42001fSingle,
                    (Some(0x42e01f), Some(1)) => LocalPayloadType::H264_42e01fNon,
                    (Some(0x42e01f), Some(0)) => LocalPayloadType::H264_42e01fSingle,
                    (Some(0x4d001f), Some(1)) => LocalPayloadType::H264_4d001fNon,
                    (Some(0x4d001f), Some(0)) => LocalPayloadType::H264_4d001fSingle,
                    (Some(0x64001f), Some(1)) => LocalPayloadType::H264_64001fNon,
                    _ => {
                        log::warn!(
                            "invalid h264 profile_level_id {:?} packetization_mode {:?}",
                            spec.format.profile_level_id,
                            spec.format.packetization_mode
                        );
                        LocalPayloadType::Unknown
                    }
                },
                str0m::format::Codec::Vp8 => LocalPayloadType::Vp8,
                str0m::format::Codec::Vp9 => match spec.format.profile_id {
                    Some(0) => LocalPayloadType::Vp9P0,
                    Some(2) => LocalPayloadType::Vp9P2,
                    _ => {
                        log::warn!("invalid vp9 profile_id {:?}", spec.format.profile_id);
                        LocalPayloadType::Unknown
                    }
                },
                _ => LocalPayloadType::Unknown,
            };

            self.local_to_remote_map[local_pt.to_u8() as usize] = pt;
            self.remote_to_local_map[pt as usize] = local_pt;
        }
    }

    pub fn to_remote(&self, local_pt: LocalPayloadType) -> u8 {
        self.local_to_remote_map[local_pt.to_u8() as usize]
    }

    pub fn to_local(&self, remote_pt: u8) -> LocalPayloadType {
        self.remote_to_local_map[remote_pt as usize]
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_payload_type_to_u8() {
        assert_eq!(LocalPayloadType::Unknown.to_u8(), 0);
        assert_eq!(LocalPayloadType::Opus.to_u8(), 1);
        assert_eq!(LocalPayloadType::Vp8.to_u8(), 2);
        assert_eq!(LocalPayloadType::Vp9P0.to_u8(), 3);
        assert_eq!(LocalPayloadType::Vp9P2.to_u8(), 4);
        assert_eq!(LocalPayloadType::H264_42001fNon.to_u8(), 5);
        assert_eq!(LocalPayloadType::H264_42001fSingle.to_u8(), 6);
        assert_eq!(LocalPayloadType::H264_42e01fNon.to_u8(), 7);
        assert_eq!(LocalPayloadType::H264_42e01fSingle.to_u8(), 8);
        assert_eq!(LocalPayloadType::H264_4d001fNon.to_u8(), 9);
        assert_eq!(LocalPayloadType::H264_4d001fSingle.to_u8(), 10);
        assert_eq!(LocalPayloadType::H264_64001fNon.to_u8(), 11);
    }
}
