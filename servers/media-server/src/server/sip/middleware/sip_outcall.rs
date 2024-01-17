//! This handler is used to handle SIP endpoint state changes.
//! It will auto end the call if the endpoint is not connected to any other endpoint.

use std::collections::{HashMap, VecDeque};

use cluster::ClusterEndpointIncomingEvent;
use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn, MediaEndpointInternalControl, MediaEndpointMiddleware, MediaEndpointMiddlewareOutput,
};
use transport::{MediaKind, TransportIncomingEvent};

enum State {
    New,
    Talking,
    End,
}

pub struct SipOutcallMiddleware {
    peer: String,
    state: State,
    actions: VecDeque<endpoint::MediaEndpointMiddlewareOutput>,
    remote_tracks: HashMap<(String, String), ()>,
}

impl SipOutcallMiddleware {
    pub fn new(peer: &str) -> Self {
        Self {
            peer: peer.to_string(),
            state: State::New,
            actions: VecDeque::new(),
            remote_tracks: HashMap::new(),
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
            ClusterEndpointIncomingEvent::PeerTrackAdded(peer, track, meta) => {
                log::info!("[SipOutcallMiddleware] peer track added: {} {}", peer, track);
                if peer != &self.peer && matches!(meta.kind, MediaKind::Audio) {
                    self.remote_tracks.insert((peer.clone(), track.clone()), ());
                }
            }
            ClusterEndpointIncomingEvent::PeerTrackRemoved(peer, track) => {
                log::info!("[SipOutcallMiddleware] peer track removed: {} {}", peer, track);
                if peer != &self.peer {
                    self.remote_tracks.remove(&(peer.clone(), track.clone()));
                    if self.remote_tracks.is_empty() {
                        log::info!("[SipOutcallMiddleware] last peer track removed => end call");
                        self.state = State::End;
                        self.actions.push_back(MediaEndpointMiddlewareOutput::Control(MediaEndpointInternalControl::ConnectionCloseRequest));
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
