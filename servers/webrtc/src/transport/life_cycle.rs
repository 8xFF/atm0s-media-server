use endpoint::{
    rpc::{LocalTrackRpcIn, RemoteTrackRpcIn},
    EndpointRpcIn,
};
use str0m::IceConnectionState;
use transport::{TransportError, TransportIncomingEvent, TransportStateEvent};

pub(crate) mod sdk;
pub(crate) mod whip;

#[derive(Debug)]
pub enum TransportLifeCycleEvent {
    New,
    Connected,
    ConnectError,
    Reconnecting,
    Reconnected,
    Failed,
    Closed,
}

pub trait TransportLifeCycle: Send {
    fn on_tick(&mut self, now_ms: u64) -> Option<TransportLifeCycleEvent>;
    fn on_webrtc_connected(&mut self, now_ms: u64) -> Option<TransportLifeCycleEvent>;
    fn on_ice_state(&mut self, now_ms: u64, ice: IceConnectionState) -> Option<TransportLifeCycleEvent>;
    fn on_data_channel(&mut self, now_ms: u64, connected: bool) -> Option<TransportLifeCycleEvent>;
}

pub fn life_cycle_event_to_event(state: Option<TransportLifeCycleEvent>) -> Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError> {
    match state {
        Some(TransportLifeCycleEvent::New) => Ok(TransportIncomingEvent::Continue),
        Some(TransportLifeCycleEvent::ConnectError) => Err(TransportError::ConnectError("unknown".to_string())),
        Some(TransportLifeCycleEvent::Connected) => Ok(TransportIncomingEvent::State(TransportStateEvent::Connected)),
        Some(TransportLifeCycleEvent::Reconnecting) => Ok(TransportIncomingEvent::State(TransportStateEvent::Reconnecting)),
        Some(TransportLifeCycleEvent::Reconnected) => Ok(TransportIncomingEvent::State(TransportStateEvent::Reconnected)),
        Some(TransportLifeCycleEvent::Closed) => Ok(TransportIncomingEvent::State(TransportStateEvent::Disconnected)),
        Some(TransportLifeCycleEvent::Failed) => Err(TransportError::ConnectionError("unknown".to_string())),
        None => Ok(TransportIncomingEvent::Continue),
    }
}
