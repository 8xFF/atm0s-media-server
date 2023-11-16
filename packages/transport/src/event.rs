use crate::{packet::MediaPacket, TrackId, TrackMeta, TrackName, TransportStats};

#[derive(PartialEq, Eq, Debug)]
pub enum RemoteTrackIncomingEvent<RR> {
    MediaPacket(MediaPacket),
    Rpc(RR),
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum RequestKeyframeKind {
    Pli,
    Fir,
}

#[derive(PartialEq, Eq, Debug)]
pub enum LocalTrackIncomingEvent<RL> {
    RequestKeyFrame(RequestKeyframeKind),
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
    EgressBitrateEstimate(u64),
}

#[derive(PartialEq, Eq, Debug)]
pub enum RemoteTrackOutgoingEvent<RR> {
    RequestKeyFrame(RequestKeyframeKind),
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
    ConfigEgressBitrate { current: u32, desired: u32 },
    LimitIngressBitrate(u32),
    Rpc(RE),
}
