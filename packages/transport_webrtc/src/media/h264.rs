use media_server_protocol::media::{H264Profile, H264Sim, MediaMeta};

const H264_NALU_TTYPE_STAP_A: u32 = 24;
const H264_NALU_TTYPE_SPS: u32 = 7;
const H264_NALU_TYPE_BITMASK: u32 = 0x1F;

pub fn parse_rtp(payload: &[u8], profile: H264Profile, rid: Option<u8>) -> Option<MediaMeta> {
    if payload.len() < 4 {
        None
    } else {
        let word = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let nalu_type = (word >> 24) & H264_NALU_TYPE_BITMASK;
        let key = (nalu_type == H264_NALU_TTYPE_STAP_A && (word & H264_NALU_TYPE_BITMASK) == H264_NALU_TTYPE_SPS) || (nalu_type == H264_NALU_TTYPE_SPS);
        //TODO getting h264 simulcast temporal layer by using frame-marking extension
        Some(MediaMeta::H264 {
            key,
            profile,
            sim: rid.map(|rid| H264Sim { spatial: rid }),
        })
    }
}

pub fn rewrite_rtp(_payload: &mut [u8], _sim: &H264Sim) {}
