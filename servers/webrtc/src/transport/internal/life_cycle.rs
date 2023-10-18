use std::collections::VecDeque;

use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn,
};
use str0m::IceConnectionState;
use transport::{ConnectErrorReason, ConnectionErrorReason, TransportError, TransportIncomingEvent, TransportStateEvent};

pub(crate) mod sdk;
pub(crate) mod whip;

#[derive(Debug, PartialEq, Eq)]
pub enum TransportLifeCycleEvent {
    New,
    Connected,
    ConnectError(ConnectErrorReason),
    Reconnecting,
    Reconnected,
    Failed(ConnectionErrorReason),
    Closed,
}

pub trait TransportLifeCycle: Send {
    fn on_tick(&mut self, now_ms: u64) -> Option<TransportLifeCycleEvent>;
    fn on_webrtc_connected(&mut self, now_ms: u64) -> Option<TransportLifeCycleEvent>;
    fn on_ice_state(&mut self, now_ms: u64, ice: IceConnectionState) -> Option<TransportLifeCycleEvent>;
    fn on_data_channel(&mut self, now_ms: u64, connected: bool) -> Option<TransportLifeCycleEvent>;
}

pub fn life_cycle_event_to_event(state: Option<TransportLifeCycleEvent>, actions: &mut VecDeque<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>>) {
    match state {
        Some(TransportLifeCycleEvent::New) => {}
        Some(TransportLifeCycleEvent::ConnectError(res)) => actions.push_back(Err(TransportError::ConnectError(res))),
        Some(TransportLifeCycleEvent::Connected) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Connected))),
        Some(TransportLifeCycleEvent::Reconnecting) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Reconnecting))),
        Some(TransportLifeCycleEvent::Reconnected) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Reconnected))),
        Some(TransportLifeCycleEvent::Closed) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Disconnected))),
        Some(TransportLifeCycleEvent::Failed(res)) => actions.push_back(Err(TransportError::ConnectionError(res))),
        None => {}
    }
}
