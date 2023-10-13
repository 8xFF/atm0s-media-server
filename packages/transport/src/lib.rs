use serde::{Deserialize, Serialize};

pub type TrackId = u16;
pub type TrackName = String;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum MediaKind {
    Audio,
    Video,
}

#[derive(Debug)]
pub enum MediaSampleRate {
    Hz8000,
    Hz16000,
    Hz32000,
    Hz48000,
    Hz96000,
    Hz192000,
    HzCustom(u32),
}

impl From<u32> for MediaSampleRate {
    fn from(value: u32) -> Self {
        match value {
            8000 => MediaSampleRate::Hz8000,
            16000 => MediaSampleRate::Hz16000,
            32000 => MediaSampleRate::Hz32000,
            48000 => MediaSampleRate::Hz48000,
            96000 => MediaSampleRate::Hz96000,
            192000 => MediaSampleRate::Hz192000,
            _ => MediaSampleRate::HzCustom(value),
        }
    }
}

impl From<MediaSampleRate> for u32 {
    fn from(value: MediaSampleRate) -> Self {
        match value {
            MediaSampleRate::Hz8000 => 8000,
            MediaSampleRate::Hz16000 => 16000,
            MediaSampleRate::Hz32000 => 32000,
            MediaSampleRate::Hz48000 => 48000,
            MediaSampleRate::Hz96000 => 96000,
            MediaSampleRate::Hz192000 => 192000,
            MediaSampleRate::HzCustom(value) => value,
        }
    }
}

#[derive(Debug)]
pub struct TrackMeta {
    pub kind: MediaKind,
    pub sample_rate: MediaSampleRate,
    pub label: Option<String>,
}

#[derive(Debug)]
pub struct TransportStats {
    rtt: u32,
    loss: u32,
    jitter: u32,
    bitrate: u32,
}

#[derive(Debug)]
pub enum MediaIncomingEvent<RM> {
    Connected,
    Reconnecting,
    Reconnected,
    Disconnected,
    Continue,
    RemoteTrackAdded(TrackName, TrackId, TrackMeta),
    RemoteTrackMedia(TrackId, MediaPacket),
    RemoteTrackRemoved(TrackName, TrackId, TrackMeta),
    LocalTrackAdded(TrackName, TrackId, TrackMeta),
    LocalTrackRemoved(TrackName, TrackId),
    Rpc(RM),
    Stats(TransportStats),
}

pub enum MediaOutgoingEvent<RM> {
    Media(TrackId, MediaPacket),
    RequestPli(TrackId),
    RequestSli(TrackId),
    RequestLimitBitrate(u32),
    Rpc(RM),
}

#[derive(Debug, Clone)]
pub struct MediaPacketExtensions {
    pub abs_send_time: Option<(i64, i64)>,
    pub transport_cc: Option<u16>, // (buf[0] << 8) | buf[1];
}

#[derive(Debug, Clone)]
pub struct MediaPacket {
    pub pt: u8,
    pub seq_no: u16,
    pub time: u32,
    pub marker: bool,
    pub ext_vals: MediaPacketExtensions,
    pub nackable: bool,
    pub payload: Vec<u8>,
}

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
pub trait MediaTransport<E, RmIn, RmOut> {
    fn on_event(&mut self, event: MediaOutgoingEvent<RmOut>) -> Result<(), MediaTransportError>;
    fn on_custom_event(&mut self, event: E) -> Result<(), MediaTransportError>;
    async fn recv(&mut self) -> Result<MediaIncomingEvent<RmIn>, MediaTransportError>;
}
