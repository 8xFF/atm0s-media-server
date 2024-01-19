//! This handler is used to handle SIP endpoint state changes.
//! It will auto accept call if the endpoint is connected to any other endpoint.
//! It will auto end the call if the endpoint is not connected to any other endpoint.

use std::collections::{HashMap, VecDeque};

use cluster::ClusterEndpointIncomingEvent;
use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn, MediaEndpointInternalControl, MediaEndpointMiddleware, MediaEndpointMiddlewareOutput,
};
use transport::TransportIncomingEvent;

const WAIT_TIMEOUT_MS: u64 = 30000;

enum State {
    New,
    WaitEndpoint { started_at: u64 },
    Talking,
    End,
}

pub struct SipIncallMiddleware {
    peer: String,
    state: State,
    actions: VecDeque<endpoint::MediaEndpointMiddlewareOutput>,
    remote_peers: HashMap<String, ()>,
}

impl SipIncallMiddleware {
    pub fn new(peer: &str) -> Self {
        Self {
            peer: peer.to_string(),
            state: State::New,
            actions: VecDeque::new(),
            remote_peers: HashMap::new(),
        }
    }
}

impl MediaEndpointMiddleware for SipIncallMiddleware {
    fn on_start(&mut self, now_ms: u64) {
        self.state = State::WaitEndpoint { started_at: now_ms };
    }

    fn on_tick(&mut self, now_ms: u64) {
        if let State::WaitEndpoint { started_at } = self.state {
            if now_ms - started_at > WAIT_TIMEOUT_MS {
                log::warn!("[SipIncallMiddleware] wait timeout => end call");
                self.state = State::End;
                self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::ConnectionCloseRequest));
            }
        }
    }

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
                    log::info!("[SipIncallMiddleware] last remote peer removed => end call");
                    self.state = State::End;
                    self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::ConnectionCloseRequest));
                }
            }
            ClusterEndpointIncomingEvent::PeerTrackAdded(peer, _track, _meta) => {
                if peer != &self.peer {
                    if matches!(self.state, State::WaitEndpoint { .. }) {
                        log::info!("[SipIncallMiddleware] first remote track added => accept call");
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
