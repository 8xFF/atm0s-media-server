use std::collections::{HashMap, VecDeque};

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, ReceiverDisconnect, ReceiverSwitch, RemoteStream, RemoteTrackRpcOut},
    EndpointRpcOut, RpcRequest,
};
use str0m::IceConnectionState;
use transport::{ConnectErrorReason, ConnectionErrorReason, LocalTrackIncomingEvent, MediaKind, TransportError, TransportIncomingEvent as TransIn, TransportOutgoingEvent, TransportStateEvent};

use crate::transport::internal::Str0mInput;

use super::{TransportLifeCycle, TransportLifeCycleAction as Out};

const AUDIO_TRACK: u16 = 0;
const VIDEO_TRACK: u16 = 1;

const CONNECT_TIMEOUT: u64 = 10000;
const RECONNECT_TIMEOUT: u64 = 30000;

fn kind_track_id(kind: MediaKind) -> u16 {
    match kind {
        MediaKind::Audio => AUDIO_TRACK,
        MediaKind::Video => VIDEO_TRACK,
    }
}

fn kind_track_name(kind: MediaKind) -> String {
    match kind {
        MediaKind::Audio => "audio_0".to_string(),
        MediaKind::Video => "video_0".to_string(),
    }
}

#[derive(Debug)]
pub enum State {
    New { at_ms: u64 },
    Connected,
    Reconnecting { at_ms: u64 },
    Failed,
}

pub struct WhepTransportLifeCycle {
    state: State,
    outputs: VecDeque<Out>,
    viewing: HashMap<MediaKind, (String, String)>,
}

impl WhepTransportLifeCycle {
    pub fn new(now_ms: u64) -> Self {
        log::info!("[WhepTransportLifeCycle] new");
        Self {
            state: State::New { at_ms: now_ms },
            outputs: VecDeque::new(),
            viewing: HashMap::new(),
        }
    }

    fn on_connected(&mut self) {
        for (kind, (peer, track)) in &self.viewing {
            let req = LocalTrackRpcIn::Switch(RpcRequest {
                req_id: 0,
                data: ReceiverSwitch {
                    id: kind_track_name(*kind),
                    priority: 1000,
                    remote: RemoteStream {
                        peer: peer.clone(),
                        stream: track.clone(),
                    },
                },
            });
            self.outputs
                .push_back(Out::ToEndpoint(TransIn::LocalTrackEvent(kind_track_id(*kind), LocalTrackIncomingEvent::Rpc(req))))
        }
    }
}

impl TransportLifeCycle for WhepTransportLifeCycle {
    fn on_tick(&mut self, now_ms: u64) {
        match &self.state {
            State::New { at_ms } => {
                if at_ms + CONNECT_TIMEOUT <= now_ms {
                    log::info!("[SdkTransportLifeCycle] on webrtc connect timeout => switched to Failed");
                    self.state = State::Failed;
                    self.outputs.push_back(Out::TransportError(TransportError::ConnectError(ConnectErrorReason::Timeout)));
                }
            }
            State::Reconnecting { at_ms } => {
                if at_ms + RECONNECT_TIMEOUT <= now_ms {
                    log::info!("[SdkTransportLifeCycle] on webrtc reconnect timeout => switched to Failed");
                    self.state = State::Failed;
                    self.outputs.push_back(Out::TransportError(TransportError::ConnectionError(ConnectionErrorReason::Timeout)));
                }
            }
            _ => {}
        }
    }

    fn on_transport_event(&mut self, now_ms: u64, event: &Str0mInput) {
        match event {
            Str0mInput::Connected => {
                self.state = State::Connected;
                log::info!("[WhepTransportLifeCycle] on webrtc connected => switched to {:?}", self.state);
                self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Connected)));
                self.on_connected();
            }
            Str0mInput::IceConnectionStateChange(ice) => match (&self.state, ice) {
                (State::Connected, IceConnectionState::Disconnected) => {
                    self.state = State::Reconnecting { at_ms: now_ms };
                    log::info!("[WhepTransportLifeCycle] on webrtc ice disconnected => switched to {:?}", self.state);
                    self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnecting)));
                }
                (State::Reconnecting { .. }, IceConnectionState::Completed) => {
                    self.state = State::Connected;
                    log::info!("[WhepTransportLifeCycle] on webrtc ice completed => switched to {:?}", self.state);
                    self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnected)));
                }
                (State::Reconnecting { .. }, IceConnectionState::Connected) => {
                    self.state = State::Connected;
                    log::info!("[WhepTransportLifeCycle] on webrtc ice connected => switched to {:?}", self.state);
                    self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnected)));
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn on_endpoint_event(&mut self, _now_ms: u64, event: &TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>) {
        match event {
            TransportOutgoingEvent::Rpc(rpc) => match rpc {
                EndpointRpcOut::TrackAdded(info) => {
                    if !self.viewing.contains_key(&info.kind) {
                        log::info!(
                            "[WhepTransportLifeCycle] on endpoint rpc TrackAdded({}/{}) => auto switch view this remote stream",
                            info.peer,
                            info.track
                        );
                        self.viewing.insert(info.kind, (info.peer.clone(), info.track.clone()));
                        let req = LocalTrackRpcIn::Switch(RpcRequest {
                            req_id: 0,
                            data: ReceiverSwitch {
                                id: kind_track_name(info.kind),
                                priority: 1000,
                                remote: RemoteStream {
                                    peer: info.peer.clone(),
                                    stream: info.track.clone(),
                                },
                            },
                        });
                        if matches!(self.state, State::Connected) {
                            self.outputs
                                .push_back(Out::ToEndpoint(TransIn::LocalTrackEvent(kind_track_id(info.kind), LocalTrackIncomingEvent::Rpc(req))))
                        }
                    }
                }
                EndpointRpcOut::TrackRemoved(info) => {
                    if let Some((peer, track)) = self.viewing.remove(&info.kind) {
                        log::info!("[WhepTransportLifeCycle] on endpoint rpc TrackRemoved({}/{}) => auto disconnect view this remote stream", peer, track);
                        let req = LocalTrackRpcIn::Disconnect(RpcRequest {
                            req_id: 0,
                            data: ReceiverDisconnect { id: kind_track_name(info.kind) },
                        });
                        if matches!(self.state, State::Connected) {
                            self.outputs
                                .push_back(Out::ToEndpoint(TransIn::LocalTrackEvent(kind_track_id(info.kind), LocalTrackIncomingEvent::Rpc(req))))
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn pop_action(&mut self) -> Option<Out> {
        self.outputs.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use endpoint::rpc::TrackInfo;
    use str0m::IceConnectionState;
    use transport::{ConnectErrorReason, ConnectionErrorReason, TransportError, TransportIncomingEvent as TransIn, TransportStateEvent};

    #[test]
    fn simple() {
        let mut life_cycle = WhepTransportLifeCycle::new(0);

        // webrtc connected should fire connected
        life_cycle.on_transport_event(0, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Connected))));
        assert_eq!(life_cycle.pop_action(), None);

        // next ice disconnect should switch to reconnecting
        life_cycle.on_transport_event(0, &Str0mInput::IceConnectionStateChange(IceConnectionState::Disconnected));
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnecting))));
        assert_eq!(life_cycle.pop_action(), None);

        // next connected should switch to reconnected
        life_cycle.on_transport_event(0, &Str0mInput::IceConnectionStateChange(IceConnectionState::Connected));
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnected))));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn connect_timeout() {
        let mut life_cycle = WhepTransportLifeCycle::new(0);

        life_cycle.on_tick(CONNECT_TIMEOUT - 1);
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(CONNECT_TIMEOUT);
        assert_eq!(life_cycle.pop_action(), Some(Out::TransportError(TransportError::ConnectError(ConnectErrorReason::Timeout))));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn reconnect_timeout() {
        let mut life_cycle = WhepTransportLifeCycle::new(0);

        // webrtc connected should not switch
        life_cycle.on_transport_event(100, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Connected))));
        assert_eq!(life_cycle.pop_action(), None);

        // next ice disconnect should switch to reconnecting
        life_cycle.on_transport_event(1000, &Str0mInput::IceConnectionStateChange(IceConnectionState::Disconnected));
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnecting))));
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(1000 + RECONNECT_TIMEOUT - 1);
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(1000 + RECONNECT_TIMEOUT);
        assert_eq!(life_cycle.pop_action(), Some(Out::TransportError(TransportError::ConnectionError(ConnectionErrorReason::Timeout))));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn auto_view_unview() {
        let mut life_cycle = WhepTransportLifeCycle::new(0);

        // webrtc connected should not switch
        life_cycle.on_transport_event(100, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Connected))));
        assert_eq!(life_cycle.pop_action(), None);

        // on endpoint RemoteAdded rpc => should request connect
        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer_id".to_string(),
                peer_hash: 0,
                track: "track_id".to_string(),
                state: None,
            })),
        );

        let event = TransIn::LocalTrackEvent(
            AUDIO_TRACK,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
                req_id: 0,
                data: ReceiverSwitch {
                    id: kind_track_name(MediaKind::Audio),
                    priority: 1000,
                    remote: RemoteStream {
                        peer: "peer_id".to_string(),
                        stream: "track_id".to_string(),
                    },
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));
        assert_eq!(life_cycle.pop_action(), None);

        // on endpint RemoteRemoved => should request disconnected
        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackRemoved(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer_id".to_string(),
                peer_hash: 0,
                track: "track_id".to_string(),
                state: None,
            })),
        );

        let event = TransIn::LocalTrackEvent(
            AUDIO_TRACK,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Disconnect(RpcRequest {
                req_id: 0,
                data: ReceiverDisconnect {
                    id: kind_track_name(MediaKind::Audio),
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));
        assert_eq!(life_cycle.pop_action(), None);
    }
}
