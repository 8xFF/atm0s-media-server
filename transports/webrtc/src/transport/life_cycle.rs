use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use transport::{TransportError, TransportIncomingEvent, TransportOutgoingEvent};

use super::internal::Str0mInput;

pub mod sdk;
pub mod whep;
pub mod whip;

#[derive(Debug, PartialEq, Eq)]
pub enum TransportLifeCycleAction {
    ToEndpoint(TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>),
    TransportError(TransportError),
}

pub trait TransportLifeCycle: Send {
    fn on_tick(&mut self, now_ms: u64);
    fn on_transport_event(&mut self, now_ms: u64, event: &Str0mInput);
    fn on_endpoint_event(&mut self, now_ms: u64, event: &TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>);
    fn pop_action(&mut self) -> Option<TransportLifeCycleAction>;
}

// pub fn life_cycle_event_to_event(state: Option<TransportLifeCycleEvent>, actions: &mut VecDeque<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>>) {
//     match state {
//         Some(TransportLifeCycleEvent::New) => {}
//         Some(TransportLifeCycleEvent::ConnectError(res)) => actions.push_back(Err(TransportError::ConnectError(res))),
//         Some(TransportLifeCycleEvent::Connected) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Connected))),
//         Some(TransportLifeCycleEvent::Reconnecting) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Reconnecting))),
//         Some(TransportLifeCycleEvent::Reconnected) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Reconnected))),
//         Some(TransportLifeCycleEvent::Closed) => actions.push_back(Ok(TransportIncomingEvent::State(TransportStateEvent::Disconnected))),
//         Some(TransportLifeCycleEvent::Failed(res)) => actions.push_back(Err(TransportError::ConnectionError(res))),
//         None => {}
//     }
// }
