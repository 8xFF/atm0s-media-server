//! Endpoint take care integrate between transport and endpoint internal logic. It don't have logic, just forward events

use std::{marker::PhantomData, time::Instant};

use media_server_protocol::{
    endpoint::{BitrateControlMode, PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackName, TrackPriority},
    media::MediaPacket,
    protobuf,
    transport::RpcResult,
};
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    return_if_some, Task, TaskSwitcher, TaskSwitcherBranch, TaskSwitcherChild,
};

use crate::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterRoomHash},
    transport::{LocalTrackId, RemoteTrackId, Transport, TransportInput, TransportOutput},
};

use internal::EndpointInternal;

use self::internal::InternalOutput;

mod internal;
mod middleware;

pub struct EndpointSession(pub u64);

#[derive(Debug, PartialEq, Eq)]
pub struct EndpointRemoteTrackConfig {
    pub priority: TrackPriority,
    pub control: BitrateControlMode,
}

impl From<protobuf::shared::sender::Config> for EndpointRemoteTrackConfig {
    fn from(value: protobuf::shared::sender::Config) -> Self {
        Self {
            priority: value.priority.into(),
            control: value.bitrate().into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointRemoteTrackReq {
    Config(EndpointRemoteTrackConfig),
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointRemoteTrackRes {
    Config(RpcResult<()>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct EndpointLocalTrackSource {
    pub peer: PeerId,
    pub track: TrackName,
}

impl From<protobuf::shared::receiver::Source> for EndpointLocalTrackSource {
    fn from(value: protobuf::shared::receiver::Source) -> Self {
        Self {
            peer: value.peer.into(),
            track: value.track.into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EndpointLocalTrackConfig {
    pub priority: TrackPriority,
    pub max_spatial: u8,
    pub max_temporal: u8,
    pub min_spatial: Option<u8>,
    pub min_temporal: Option<u8>,
}

impl From<protobuf::shared::receiver::Config> for EndpointLocalTrackConfig {
    fn from(value: protobuf::shared::receiver::Config) -> Self {
        Self {
            priority: value.priority.into(),
            max_spatial: value.max_spatial as u8,
            max_temporal: value.max_temporal as u8,
            min_spatial: value.min_spatial.map(|m| m as u8),
            min_temporal: value.min_temporal.map(|m| m as u8),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointLocalTrackReq {
    Attach(EndpointLocalTrackSource, EndpointLocalTrackConfig),
    Detach(),
    Config(EndpointLocalTrackConfig),
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointLocalTrackRes {
    Attach(RpcResult<()>),
    Detach(RpcResult<()>),
    Config(RpcResult<()>),
}

#[derive(Debug, PartialEq, Eq, derive_more::From)]
pub struct EndpointReqId(pub u32);

/// This is control APIs, which is used to control server from Endpoint SDK
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointReq {
    JoinRoom(RoomId, PeerId, PeerMeta, RoomInfoPublish, RoomInfoSubscribe),
    LeaveRoom,
    SubscribePeer(PeerId),
    UnsubscribePeer(PeerId),
    RemoteTrack(RemoteTrackId, EndpointRemoteTrackReq),
    LocalTrack(LocalTrackId, EndpointLocalTrackReq),
}

/// This is response, which is used to send response back to Endpoint SDK
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointRes {
    JoinRoom(RpcResult<()>),
    LeaveRoom(RpcResult<()>),
    SubscribePeer(RpcResult<()>),
    UnsubscribePeer(RpcResult<()>),
    RemoteTrack(RemoteTrackId, EndpointRemoteTrackRes),
    LocalTrack(LocalTrackId, EndpointLocalTrackRes),
}

/// This is used for controlling the local track, which is sent from endpoint
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointLocalTrackEvent {
    Media(MediaPacket),
    Status(protobuf::shared::receiver::Status),
}

/// This is used for controlling the remote track, which is sent from endpoint
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointRemoteTrackEvent {
    RequestKeyFrame,
    LimitBitrateBps { min: u64, max: u64 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointEvent {
    PeerJoined(PeerId, PeerMeta),
    PeerLeaved(PeerId, PeerMeta),
    PeerTrackStarted(PeerId, TrackName, TrackMeta),
    PeerTrackStopped(PeerId, TrackName, TrackMeta),
    RemoteMediaTrack(RemoteTrackId, EndpointRemoteTrackEvent),
    LocalMediaTrack(LocalTrackId, EndpointLocalTrackEvent),
    /// Egress est params
    BweConfig {
        current: u64,
        desired: u64,
    },
    /// This session will be disconnect after some seconds
    GoAway(u8, Option<String>),
}

pub enum EndpointInput<Ext> {
    Net(BackendIncoming),
    Cluster(ClusterEndpointEvent),
    Ext(Ext),
    Close,
}

pub enum EndpointOutput<Ext> {
    Net(BackendOutgoing),
    Cluster(ClusterRoomHash, ClusterEndpointControl),
    Ext(Ext),
    Continue,
    Destroy,
}

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(usize)]
enum TaskType {
    Transport = 0,
    Internal = 1,
}

pub struct EndpointCfg {
    pub max_egress_bitrate: u64,
    pub max_ingress_bitrate: u64,
}

pub struct Endpoint<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> {
    transport: TaskSwitcherBranch<T, TransportOutput<ExtOut>>,
    internal: TaskSwitcherBranch<EndpointInternal, InternalOutput>,
    switcher: TaskSwitcher,
    _tmp: PhantomData<(ExtIn, ExtOut)>,
}

impl<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> Endpoint<T, ExtIn, ExtOut> {
    pub fn new(cfg: EndpointCfg, transport: T) -> Self {
        Self {
            transport: TaskSwitcherBranch::new(transport, TaskType::Transport),
            internal: TaskSwitcherBranch::new(EndpointInternal::new(cfg), TaskType::Internal),
            switcher: TaskSwitcher::new(2),
            _tmp: PhantomData::default(),
        }
    }
}

impl<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> Task<EndpointInput<ExtIn>, EndpointOutput<ExtOut>> for Endpoint<T, ExtIn, ExtOut>
where
    T::Time: From<Instant>,
{
    fn on_tick(&mut self, now: Instant) {
        self.internal.input(&mut self.switcher).on_tick(now);
        self.transport.input(&mut self.switcher).on_tick(now);
    }

    fn on_event(&mut self, now: Instant, input: EndpointInput<ExtIn>) {
        match input {
            EndpointInput::Net(net) => {
                self.transport.input(&mut self.switcher).on_input(now, TransportInput::Net(net));
            }
            EndpointInput::Ext(ext) => {
                self.transport.input(&mut self.switcher).on_input(now, TransportInput::Ext(ext));
            }
            EndpointInput::Cluster(event) => {
                self.internal.input(&mut self.switcher).on_cluster_event(now, event);
            }
            EndpointInput::Close => {
                self.transport.input(&mut self.switcher).on_input(now, TransportInput::Close);
            }
        }
    }

    fn on_shutdown(&mut self, now: Instant) {
        self.transport.input(&mut self.switcher).on_input(now, TransportInput::Close);
    }
}

impl<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> TaskSwitcherChild<EndpointOutput<ExtOut>> for Endpoint<T, ExtIn, ExtOut>
where
    T::Time: From<Instant>,
{
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<EndpointOutput<ExtOut>> {
        loop {
            match self.switcher.current()?.try_into().ok()? {
                TaskType::Internal => {
                    if let Some(out) = self.internal.pop_output(now, &mut self.switcher) {
                        return_if_some!(self.process_internal_output(now, out));
                    }
                }
                TaskType::Transport => {
                    if let Some(out) = self.transport.pop_output(now.into(), &mut self.switcher) {
                        return_if_some!(self.process_transport_output(now, out));
                    }
                }
            }
        }
    }
}

impl<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> Endpoint<T, ExtIn, ExtOut> {
    fn process_transport_output(&mut self, now: Instant, out: TransportOutput<ExtOut>) -> Option<EndpointOutput<ExtOut>> {
        match out {
            TransportOutput::Event(event) => {
                self.internal.input(&mut self.switcher).on_transport_event(now, event);
                None
            }
            TransportOutput::Ext(ext) => Some(EndpointOutput::Ext(ext)),
            TransportOutput::Net(net) => Some(EndpointOutput::Net(net)),
            TransportOutput::RpcReq(req_id, req) => {
                self.internal.input(&mut self.switcher).on_transport_rpc(now, req_id, req);
                None
            }
        }
    }

    fn process_internal_output(&mut self, now: Instant, out: internal::InternalOutput) -> Option<EndpointOutput<ExtOut>> {
        match out {
            InternalOutput::Event(event) => {
                self.transport.input(&mut self.switcher).on_input(now, TransportInput::Endpoint(event));
                None
            }
            InternalOutput::RpcRes(req_id, res) => {
                self.transport.input(&mut self.switcher).on_input(now, TransportInput::RpcRes(req_id, res));
                None
            }
            InternalOutput::Cluster(room, control) => Some(EndpointOutput::Cluster(room, control)),
            InternalOutput::Destroy => Some(EndpointOutput::Destroy),
        }
    }
}

#[cfg(test)]
mod tests {
    //TODO should forward event from transport to internal
    //TODO should forward event from internal to transport
    //TODO should output cluster events
}
