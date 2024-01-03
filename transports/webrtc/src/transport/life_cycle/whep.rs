use std::collections::{HashMap, VecDeque};

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, ReceiverDisconnect, ReceiverSwitch, RemoteStream, RemoteTrackRpcOut},
    EndpointRpcOut, RpcRequest,
};
use str0m::{media::Direction, IceConnectionState};
use transport::{
    ConnectErrorReason, ConnectionErrorReason, LocalTrackIncomingEvent, MediaKind, TrackId, TransportError, TransportIncomingEvent as TransIn, TransportOutgoingEvent, TransportStateEvent,
};

use crate::transport::internal::{utils::to_transport_kind, Str0mInput};

use super::{TransportLifeCycle, TransportLifeCycleAction as Out};

const CONNECT_TIMEOUT: u64 = 10000;
const RECONNECT_TIMEOUT: u64 = 30000;

fn kind_track_name(kind: MediaKind) -> String {
    match kind {
        MediaKind::Audio => "audio_0".to_string(),
        MediaKind::Video => "video_0".to_string(),
    }
}

#[derive(Debug)]
enum State {
    New { at_ms: u64 },
    Connected,
    Reconnecting { at_ms: u64 },
    Failed,
}

struct LocalTrack {
    track_id: TrackId,
    viewing: Option<(String, String)>,
}

pub struct WhepTransportLifeCycle {
    state: State,
    outputs: VecDeque<Out>,
    remote_tracks: HashMap<MediaKind, Vec<(String, String)>>,
    local_tracks: HashMap<MediaKind, LocalTrack>,
}

impl WhepTransportLifeCycle {
    pub fn new(now_ms: u64) -> Self {
        log::info!("[WhepTransportLifeCycle] new");
        Self {
            state: State::New { at_ms: now_ms },
            outputs: VecDeque::new(),
            remote_tracks: HashMap::from([(MediaKind::Audio, vec![]), (MediaKind::Video, vec![])]),
            local_tracks: HashMap::new(),
        }
    }

    fn connect_waiting_tracks(&mut self) {
        for (kind, slot) in &mut self.local_tracks {
            if slot.viewing.is_some() {
                continue;
            }
            if let Some((peer, track)) = self.remote_tracks.get(kind).expect("Should has").first() {
                log::info!("[WhepTransportLifeCycle] auto switch view this remote stream ({}/{})", peer, track);
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
                slot.viewing = Some((peer.clone(), track.clone()));
                self.outputs.push_back(Out::ToEndpoint(TransIn::LocalTrackEvent(slot.track_id, LocalTrackIncomingEvent::Rpc(req))))
            }
        }
    }
}

impl TransportLifeCycle for WhepTransportLifeCycle {
    fn on_tick(&mut self, now_ms: u64) {
        self.connect_waiting_tracks();

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
                self.connect_waiting_tracks();
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
            Str0mInput::MediaAdded(direction, track_id, kind, _) => {
                let kind = to_transport_kind(*kind);
                log::info!("[WhepTransportLifeCycle] added media {kind:?} {direction}");
                if direction.eq(&Direction::SendOnly) {
                    if !self.local_tracks.contains_key(&kind) {
                        self.local_tracks.insert(kind, LocalTrack { track_id: *track_id, viewing: None });
                        //dont call self.connect_waiting_tracks(); here because is if before internal logic => will view request before LocalTrack added
                    }
                }
            }
            _ => {}
        }
    }

    fn on_endpoint_event(&mut self, _now_ms: u64, event: &TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>) {
        match event {
            TransportOutgoingEvent::Rpc(rpc) => match rpc {
                EndpointRpcOut::TrackAdded(info) => {
                    let tracks = self.remote_tracks.get_mut(&info.kind).expect("Must has");
                    tracks.push((info.peer.clone(), info.track.clone()));
                    log::info!("[WhepTransportLifeCycle] on endpoint rpc TrackAdded({}/{}) => remotes {:?}", info.peer, info.track, tracks);
                    self.connect_waiting_tracks();
                }
                EndpointRpcOut::TrackRemoved(info) => {
                    let tracks = self.remote_tracks.get_mut(&info.kind).expect("Must has");
                    tracks.retain(|(peer, track)| !peer.eq(&info.peer) || !track.eq(&info.track));
                    log::info!("[WhepTransportLifeCycle] on endpoint rpc TrackRemoved({}/{}) => remotes {:?}", info.peer, info.track, tracks);

                    if let Some(slot) = self.local_tracks.get_mut(&info.kind) {
                        if let Some((peer, track)) = &slot.viewing {
                            if peer.eq(&info.peer) && track.eq(&info.track) {
                                log::info!("[WhepTransportLifeCycle] on endpoint rpc TrackRemoved({}/{}) => auto disconnect view this remote stream", peer, track);
                                slot.viewing = None;
                                let req = LocalTrackRpcIn::Disconnect(RpcRequest {
                                    req_id: 0,
                                    data: ReceiverDisconnect { id: kind_track_name(info.kind) },
                                });
                                self.outputs.push_back(Out::ToEndpoint(TransIn::LocalTrackEvent(slot.track_id, LocalTrackIncomingEvent::Rpc(req))));

                                self.connect_waiting_tracks();
                            }
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
    use crate::transport::mid_convert::generate_mid;

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
        life_cycle.on_transport_event(100, &Str0mInput::MediaAdded(Direction::SendOnly, 0, str0m::media::MediaKind::Audio, None));
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer1".to_string(),
                peer_hash: 0,
                track: "track_audio".to_string(),
                state: None,
            })),
        );

        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer2".to_string(),
                peer_hash: 0,
                track: "track_audio".to_string(),
                state: None,
            })),
        );

        let event = TransIn::LocalTrackEvent(
            0,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
                req_id: 0,
                data: ReceiverSwitch {
                    id: kind_track_name(MediaKind::Audio),
                    priority: 1000,
                    remote: RemoteStream {
                        peer: "peer1".to_string(),
                        stream: "track_audio".to_string(),
                    },
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));
        assert_eq!(life_cycle.pop_action(), None);

        // on endpoint RemoteRemoved => should request disconnected
        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackRemoved(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer1".to_string(),
                peer_hash: 0,
                track: "track_audio".to_string(),
                state: None,
            })),
        );

        let event = TransIn::LocalTrackEvent(
            0,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Disconnect(RpcRequest {
                req_id: 0,
                data: ReceiverDisconnect {
                    id: kind_track_name(MediaKind::Audio),
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));

        // after disconnect must switch to remain remotes
        let event = TransIn::LocalTrackEvent(
            0,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
                req_id: 0,
                data: ReceiverSwitch {
                    id: kind_track_name(MediaKind::Audio),
                    priority: 1000,
                    remote: RemoteStream {
                        peer: "peer2".to_string(),
                        stream: "track_audio".to_string(),
                    },
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));

        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn auto_view_unview_lazy() {
        let mut life_cycle = WhepTransportLifeCycle::new(0);

        // webrtc connected should not switch
        life_cycle.on_transport_event(100, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Connected))));
        assert_eq!(life_cycle.pop_action(), None);

        // on endpoint RemoteAdded rpc but dont have local track => should not request switch
        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer1".to_string(),
                peer_hash: 0,
                track: "track_audio".to_string(),
                state: None,
            })),
        );

        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackAdded(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer2".to_string(),
                peer_hash: 0,
                track: "track_audio".to_string(),
                state: None,
            })),
        );

        // after has local track still not switch, need to wait for next tick
        life_cycle.on_transport_event(100, &Str0mInput::MediaAdded(Direction::SendOnly, 0, str0m::media::MediaKind::Audio, None));
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(200);
        let event = TransIn::LocalTrackEvent(
            0,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
                req_id: 0,
                data: ReceiverSwitch {
                    id: kind_track_name(MediaKind::Audio),
                    priority: 1000,
                    remote: RemoteStream {
                        peer: "peer1".to_string(),
                        stream: "track_audio".to_string(),
                    },
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));
        assert_eq!(life_cycle.pop_action(), None);

        // on endpoint RemoteRemoved => should request disconnected
        life_cycle.on_endpoint_event(
            1000,
            &TransportOutgoingEvent::Rpc(EndpointRpcOut::TrackRemoved(TrackInfo {
                kind: MediaKind::Audio,
                peer: "peer1".to_string(),
                peer_hash: 0,
                track: "track_audio".to_string(),
                state: None,
            })),
        );

        let event = TransIn::LocalTrackEvent(
            0,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Disconnect(RpcRequest {
                req_id: 0,
                data: ReceiverDisconnect {
                    id: kind_track_name(MediaKind::Audio),
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));

        // after disconnect must switch to remain remotes
        let event = TransIn::LocalTrackEvent(
            0,
            LocalTrackIncomingEvent::Rpc(LocalTrackRpcIn::Switch(RpcRequest {
                req_id: 0,
                data: ReceiverSwitch {
                    id: kind_track_name(MediaKind::Audio),
                    priority: 1000,
                    remote: RemoteStream {
                        peer: "peer2".to_string(),
                        stream: "track_audio".to_string(),
                    },
                },
            })),
        );
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(event)));

        assert_eq!(life_cycle.pop_action(), None);
    }
}
