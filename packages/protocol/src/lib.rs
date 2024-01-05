pub mod media_event_logs {
    use prost::Message;
    use std::vec;

    pub type MediaEndpointLogEvent = media_endpoint_log_request::Event;
    pub type MediaSessionEvent = session_event::Event;

    include!(concat!(env!("OUT_DIR"), "/atm0s.media_endpoint_log.rs"));
    impl From<MediaEndpointLogRequest> for Vec<u8> {
        fn from(val: MediaEndpointLogRequest) -> Self {
            val.encode_to_vec()
        }
    }

    impl TryFrom<&[u8]> for MediaEndpointLogRequest {
        type Error = prost::DecodeError;

        fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
            Self::decode(value)
        }
    }

    impl From<MediaEndpointLogEvent> for Vec<u8> {
        fn from(val: MediaEndpointLogEvent) -> Self {
            let mut buf = vec![];
            val.encode(&mut buf);
            buf
        }
    }

    impl From<f32> for F32p2 {
        fn from(val: f32) -> Self {
            Self { value: (val * 100.0) as u32 }
        }
    }

    impl From<F32p2> for f32 {
        fn from(value: F32p2) -> Self {
            value.value as f32 / 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::media_event_logs::*;

    #[test]
    fn test_f32p2_conversion() {
        let value: f32 = 3.14;
        let f32p2_value: F32p2 = value.into();
        let converted_value: f32 = f32p2_value.into();

        assert_eq!(value, converted_value);
    }
}
