use transport::H264Simulcast;

const H264_NALU_TTYPE_STAP_A: u32 = 24;
const H264_NALU_TTYPE_SPS: u32 = 7;
const H264_NALU_TYPE_BITMASK: u32 = 0x1F;

pub fn payload_parse(payload: &[u8], rid: Option<u16>) -> (bool, Option<H264Simulcast>) {
    if payload.len() < 4 {
        (false, None)
    } else {
        let word = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let nalu_type = (word >> 24) & H264_NALU_TYPE_BITMASK;
        let is_key = (nalu_type == H264_NALU_TTYPE_STAP_A && (word & H264_NALU_TYPE_BITMASK) == H264_NALU_TTYPE_SPS) || (nalu_type == H264_NALU_TTYPE_SPS);
        //TODO getting h264 simulcast temporal layer by using frame-marking extension
        (is_key, rid.map(|layer| H264Simulcast { sparital: layer as u8 }))
    }
}

//TODO test this
