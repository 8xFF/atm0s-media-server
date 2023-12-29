use prost::Message;

pub mod media_event_logs {
    use std::vec;

    use prost::Message;

    pub type MediaEndpointLogRequest = media_endpoint_log_event::Event;
    pub type MediaSessionEvent = session_event::Event;

    include!(concat!(env!("OUT_DIR"), "/atm0s.media_endpoint_log.rs"));
    impl From<MediaEndpointLogEvent> for Vec<u8> {
        fn from(val: MediaEndpointLogEvent) -> Self {
            val.encode_to_vec()
        }
    }

    impl TryFrom<&[u8]> for MediaEndpointLogEvent {
        type Error = prost::DecodeError;

        fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
            Self::decode(value)
        }
    }

    impl From<MediaEndpointLogRequest> for Vec<u8> {
        fn from(val: MediaEndpointLogRequest) -> Self {
            let mut buf = vec![];
            val.encode(&mut buf);
            buf
        }
    }
}

pub struct Protocol {}

impl Protocol {
    pub fn to_vec<OB: Message>(ob: &OB) -> Vec<u8> {
        ob.encode_to_vec()
    }
}
