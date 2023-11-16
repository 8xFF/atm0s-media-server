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
        (is_key, rid.map(|layer| H264Simulcast { spatial: layer as u8 }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_parse() {
        // Test case 1: Payload length less than 4
        let payload = [0u8; 3];
        let (is_key, simulcast) = payload_parse(&payload, None);
        assert_eq!(is_key, false);
        assert_eq!(simulcast, None);

        // TODO: Test case 2: SPS NAL unit

        // Test case 3: Non-SPS NAL unit
        let payload = [0x00, 0x00, 0x00, 0x01, 0x23];
        let (is_key, simulcast) = payload_parse(&payload, Some(2));
        assert_eq!(is_key, false);
        assert_eq!(simulcast, Some(H264Simulcast { spatial: 2 }));
    }
}
