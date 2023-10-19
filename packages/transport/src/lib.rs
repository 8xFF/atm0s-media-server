use serde::{Deserialize, Serialize};

pub type TrackId = u16;
pub type TrackName = String;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum MediaKind {
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "video")]
    Video,
}

#[derive(PartialEq, Eq, Debug)]
pub enum MediaSampleRate {
    Hz8000,
    Hz16000,
    Hz32000,
    Hz48000,
    Hz90000, //For video
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
            90000 => MediaSampleRate::Hz90000,
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
            MediaSampleRate::Hz90000 => 90000,
            MediaSampleRate::Hz96000 => 96000,
            MediaSampleRate::Hz192000 => 192000,
            MediaSampleRate::HzCustom(value) => value,
        }
    }
}

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

    pub fn new_audio(label: Option<String>) -> Self {
        Self {
            kind: MediaKind::Audio,
            sample_rate: MediaSampleRate::Hz48000,
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
pub enum RemoteTrackIncomingEvent<RR> {
    MediaPacket(MediaPacket),
    Rpc(RR),
}

#[derive(PartialEq, Eq, Debug)]
pub enum LocalTrackIncomingEvent<RL> {
    RequestKeyFrame,
    Rpc(RL),
}

#[derive(PartialEq, Eq, Debug)]
pub enum TransportStateEvent {
    Connected,
    Reconnecting,
    Reconnected,
    Disconnected,
}

#[derive(PartialEq, Eq, Debug)]
pub enum TransportIncomingEvent<RE, RR, RL> {
    State(TransportStateEvent),
    Continue,
    RemoteTrackAdded(TrackName, TrackId, TrackMeta),
    RemoteTrackEvent(TrackId, RemoteTrackIncomingEvent<RR>),
    RemoteTrackRemoved(TrackName, TrackId),
    LocalTrackAdded(TrackName, TrackId, TrackMeta),
    LocalTrackEvent(TrackId, LocalTrackIncomingEvent<RL>),
    LocalTrackRemoved(TrackName, TrackId),
    Rpc(RE),
    Stats(TransportStats),
}

#[derive(PartialEq, Eq, Debug)]
pub enum RemoteTrackOutgoingEvent<RR> {
    RequestKeyFrame,
    Rpc(RR),
}

#[derive(PartialEq, Eq, Debug)]
pub enum LocalTrackOutgoingEvent<RL> {
    MediaPacket(MediaPacket),
    Rpc(RL),
}

#[derive(PartialEq, Eq, Debug)]
pub enum TransportOutgoingEvent<RE, RR, RL> {
    RemoteTrackEvent(TrackId, RemoteTrackOutgoingEvent<RR>),
    LocalTrackEvent(TrackId, LocalTrackOutgoingEvent<RL>),
    RequestLimitBitrate(u32),
    Rpc(RE),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPacketExtensions {
    pub abs_send_time: Option<(i64, i64)>,
    pub transport_cc: Option<u16>, // (buf[0] << 8) | buf[1];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPacket {
    pub pt: u8,
    pub seq_no: u16,
    pub time: u32,
    pub marker: bool,
    pub ext_vals: MediaPacketExtensions,
    pub nackable: bool,
    pub payload: Vec<u8>,
}

impl MediaPacket {
    pub fn default_audio(seq_no: u16, time: u32, payload: Vec<u8>) -> Self {
        Self {
            pt: 111,
            seq_no,
            time,
            marker: false,
            ext_vals: MediaPacketExtensions {
                abs_send_time: None,
                transport_cc: None,
            },
            nackable: false,
            payload,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum ConnectErrorReason {
    Timeout,
}

#[derive(PartialEq, Eq, Debug)]
pub enum ConnectionErrorReason {
    Timeout,
}

#[derive(PartialEq, Eq, Debug)]
pub enum TransportRuntimeError {
    RpcInvalid,
    TrackIdNotFound,
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
}
