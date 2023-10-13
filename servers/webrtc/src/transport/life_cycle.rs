use endpoint::EndpointRpcIn;
use str0m::IceConnectionState;
use transport::{MediaIncomingEvent, MediaTransportError};

pub(crate) mod sdk;
pub(crate) mod whip;

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
    fn on_webrtc_connected(&mut self) -> Option<TransportLifeCycleEvent>;
    fn on_ice_state(&mut self, ice: IceConnectionState) -> Option<TransportLifeCycleEvent>;
    fn on_data_channel(&mut self, connected: bool) -> Option<TransportLifeCycleEvent>;
}

pub fn life_cycle_event_to_event(state: Option<TransportLifeCycleEvent>) -> Result<MediaIncomingEvent<EndpointRpcIn>, MediaTransportError> {
    match state {
        Some(TransportLifeCycleEvent::New) => Ok(MediaIncomingEvent::Continue),
        Some(TransportLifeCycleEvent::ConnectError) => Err(MediaTransportError::ConnectError("unknown".to_string())),
        Some(TransportLifeCycleEvent::Connected) => Ok(MediaIncomingEvent::Connected),
        Some(TransportLifeCycleEvent::Reconnecting) => Ok(MediaIncomingEvent::Reconnecting),
        Some(TransportLifeCycleEvent::Reconnected) => Ok(MediaIncomingEvent::Reconnected),
        Some(TransportLifeCycleEvent::Closed) => Ok(MediaIncomingEvent::Disconnected),
        Some(TransportLifeCycleEvent::Failed) => Err(MediaTransportError::ConnectionError("unknown".to_string())),
        None => Ok(MediaIncomingEvent::Continue),
    }
}
