use cluster::{ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent};
use transport::{TransportError, TransportIncomingEvent, TransportOutgoingEvent};

use crate::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};

pub mod logger;

pub enum MediaEndpointMiddlewareOutput {
    Endpoint(TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>),
    Cluster(ClusterEndpointOutgoingEvent),
}

pub trait MediaEndpointMiddleware: Send + Sync {
    fn on_start(&mut self, now_ms: u64);
    fn on_tick(&mut self, now_ms: u64);
    /// return true if event is consumed
    fn on_transport(&mut self, now_ms: u64, event: &TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>) -> bool;
    /// return true if event is consumed
    fn on_transport_error(&mut self, now_ms: u64, error: &TransportError) -> bool;
    /// return true if event is consumed
    fn on_cluster(&mut self, now_ms: u64, event: &ClusterEndpointIncomingEvent) -> bool;
    fn pop_action(&mut self) -> Option<MediaEndpointMiddlewareOutput>;
}
