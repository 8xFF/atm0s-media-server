//! This handler is used to handle SIP endpoint state changes.
//! It will auto end the call if the endpoint is not connected to any other endpoint.

use std::collections::{HashMap, VecDeque};

use cluster::ClusterEndpointIncomingEvent;
use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn, MediaEndpointInternalControl, MediaEndpointMiddleware, MediaEndpointMiddlewareOutput,
};
use transport::TransportIncomingEvent;

enum State {
    New,
    Talking,
    End,
}

pub struct SipOutcallMiddleware {
    peer: String,
    state: State,
    actions: VecDeque<endpoint::MediaEndpointMiddlewareOutput>,
    remote_peers: HashMap<String, ()>,
}

impl SipOutcallMiddleware {
    pub fn new(peer: &str) -> Self {
        Self {
            peer: peer.to_string(),
            state: State::New,
            actions: VecDeque::new(),
            remote_peers: HashMap::new(),
        }
    }
}

impl MediaEndpointMiddleware for SipOutcallMiddleware {
    fn on_start(&mut self, _now_ms: u64) {}

    fn on_tick(&mut self, _now_ms: u64) {}

    fn on_transport(&mut self, _now_ms: u64, _event: &TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>) -> bool {
        false
    }

    fn on_transport_error(&mut self, _now_ms: u64, _error: &transport::TransportError) -> bool {
        false
    }

    fn on_cluster(&mut self, _now_ms: u64, event: &ClusterEndpointIncomingEvent) -> bool {
        match event {
            ClusterEndpointIncomingEvent::PeerAdded(peer, _meta) => {
                if peer != &self.peer {
                    self.remote_peers.insert(peer.clone(), ());
                }
            }
            ClusterEndpointIncomingEvent::PeerRemoved(peer) => {
                if self.remote_peers.remove(peer).is_some() && self.remote_peers.is_empty() {
                    log::info!("[SipOutcallMiddleware] last remote peer removed => end call");
                    self.state = State::End;
                    self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::ConnectionCloseRequest));
                }
            }
            ClusterEndpointIncomingEvent::PeerTrackAdded(peer, _track, _meta) => {
                if peer != &self.peer {
                    if matches!(self.state, State::New { .. }) {
                        log::info!("[SipIutcallMiddleware] first remote track added => accept call");
                        self.state = State::Talking;
                        self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::ConnectionAcceptRequest));
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn pop_action(&mut self, _now_ms: u64) -> Option<MediaEndpointMiddlewareOutput> {
        self.actions.pop_front()
    }

    fn before_drop(&mut self, _now_ms: u64) {}
}
