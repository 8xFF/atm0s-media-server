mod codec;
mod event;
mod kind;
mod packet;
mod samplerate;

pub use codec::*;
pub use event::*;
pub use kind::*;
pub use packet::*;
pub use samplerate::*;

pub type TrackId = u16;
pub type TrackName = String;

#[derive(PartialEq, Eq, Debug)]
pub struct TrackMeta {
    pub kind: MediaKind,
    pub sample_rate: MediaSampleRate,
    pub label: Option<String>,
}

impl TrackMeta {
    pub fn new(kind: MediaKind, sample_rate: MediaSampleRate, label: Option<String>) -> Self {
        Self { kind, sample_rate, label }
    }

    pub fn from_kind(kind: MediaKind, label: Option<String>) -> Self {
        Self {
            kind,
            sample_rate: match kind {
                MediaKind::Audio => MediaSampleRate::Hz48000,
                MediaKind::Video => MediaSampleRate::Hz90000,
            },
            label,
        }
    }

    pub fn new_audio(label: Option<String>) -> Self {
        Self {
            kind: MediaKind::Audio,
            sample_rate: MediaSampleRate::Hz48000,
            label,
        }
    }

    pub fn new_video(label: Option<String>) -> Self {
        Self {
            kind: MediaKind::Video,
            sample_rate: MediaSampleRate::Hz90000,
            label,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct TransportStats {
    rtt: u32,
    loss: u32,
    jitter: u32,
    bitrate: u32,
}

#[derive(PartialEq, Eq, Debug)]
pub enum ConnectErrorReason {
    Timeout,
    ///This is used with SIP transport if the SIP server return 4xx or 5xx
    Rejected,
}

#[derive(PartialEq, Eq, Debug)]
pub enum ConnectionErrorReason {
    Timeout,
}

#[derive(PartialEq, Eq, Debug)]
pub enum TransportRuntimeError {
    RpcInvalid,
    TrackIdNotFound,
    RtpInvalid,
    ProtocolError,
}

#[derive(PartialEq, Eq, Debug)]
pub enum TransportError {
    /// This is connect error, the transport should try to reconnect if need
    ConnectError(ConnectErrorReason),
    /// This is a fatal error, the transport should be closed
    ConnectionError(ConnectionErrorReason),
    /// Network error
    NetworkError,
    /// Runtime error
    RuntimeError(TransportRuntimeError),
}

impl ToString for TransportError {
    fn to_string(&self) -> String {
        //TODO
        "MediaTransportError_ToString".to_string()
    }
}

#[async_trait::async_trait]
pub trait Transport<E, RmIn, RrIn, RlIn, RmOut, RrOut, RlOut> {
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError>;
    fn on_event(&mut self, now_ms: u64, event: TransportOutgoingEvent<RmOut, RrOut, RlOut>) -> Result<(), TransportError>;
    fn on_custom_event(&mut self, now_ms: u64, event: E) -> Result<(), TransportError>;
    async fn recv(&mut self, now_ms: u64) -> Result<TransportIncomingEvent<RmIn, RrIn, RlIn>, TransportError>;
    async fn close(&mut self, now_ms: u64);
}
