use std::marker::PhantomData;

use prost::Message;

pub mod media_event_logs {
    use std::vec;

    use prost::Message;

    pub type MediaEndpointLogEvent = media_endpoint_log_request::Event;
    pub type MediaSessionEvent = session_event::Event;

    include!(concat!(env!("OUT_DIR"), "/atm0s.media_endpoint_log.rs"));
    impl From<MediaEndpointLogRequest> for Vec<u8> {
        fn from(val: MediaEndpointLogRequest) -> Self {
            val.encode_to_vec()
        }
    }

    impl TryFrom<&[u8]> for MediaEndpointLogRequest{
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
}
