pub type TrackId = u16;

pub enum MediaIncomingEvent {
    Connected,
    Reconnecting,
    Reconnected,
    Disconnected,
    Continue,
    Media(TrackId, RtpPacket),
    Data(String),
    Stats { rtt: u32, loss: u32, jitter: u32, bitrate: u32 },
}

pub enum MediaOutgoingEvent {
    Media(TrackId, RtpPacket),
    RequestPli(),
    RequestSli(),
    RequestLimitBitrate(u32),
    Data(String),
}

pub struct RtpPacket {}

#[derive(Debug)]
pub enum MediaTransportError {
    /// This is connect error, the transport should try to reconnect if need
    ConnectError(String),
    /// This is a fatal error, the transport should be closed
    ConnectionError(String),
    NotImplement,
    Other(String),
}

impl ToString for MediaTransportError {
    fn to_string(&self) -> String {
        //TODO
        "MediaTransportError_ToString".to_string()
    }
}

#[async_trait::async_trait]
pub trait MediaTransport<E> {
    fn on_event(&mut self, event: MediaOutgoingEvent) -> Result<(), MediaTransportError>;
    fn on_custom_event(&mut self, event: E) -> Result<(), MediaTransportError>;
    async fn recv(&mut self) -> Result<MediaIncomingEvent, MediaTransportError>;
}
