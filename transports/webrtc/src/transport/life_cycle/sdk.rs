use std::collections::VecDeque;

use str0m::IceConnectionState;
use transport::{ConnectErrorReason, ConnectionErrorReason, TransportError, TransportIncomingEvent as TransIn, TransportOutgoingEvent, TransportStateEvent};

use crate::transport::internal::Str0mInput;

use super::{TransportLifeCycle, TransportLifeCycleAction as Out};

const CONNECT_TIMEOUT: u64 = 10000;
const RECONNECT_TIMEOUT: u64 = 30000;

#[derive(Debug)]
enum State {
    New { at_ms: u64 },
    Connected { datachannel: bool, at_ms: u64 },
    Reconnecting { datachannel: bool, at_ms: u64 },
    Failed,
    Closed,
}

pub struct SdkTransportLifeCycle {
    state: State,
    outputs: VecDeque<Out>,
}

impl SdkTransportLifeCycle {
    pub fn new(now_ms: u64) -> Self {
        log::info!("[SdkTransportLifeCycle] new");
        Self {
            state: State::New { at_ms: now_ms },
            outputs: VecDeque::new(),
        }
    }
}

impl TransportLifeCycle for SdkTransportLifeCycle {
    fn on_tick(&mut self, now_ms: u64) {
        match self.state {
            State::New { at_ms } => {
                if at_ms + CONNECT_TIMEOUT <= now_ms {
                    log::info!("[SdkTransportLifeCycle] on webrtc connect timeout => switched to Failed");
                    self.state = State::Failed;
                    self.outputs.push_back(Out::TransportError(TransportError::ConnectError(ConnectErrorReason::Timeout)));
                }
            }
            State::Connected { datachannel, at_ms } => {
                if !datachannel && at_ms + CONNECT_TIMEOUT <= now_ms {
                    log::info!("[SdkTransportLifeCycle] on webrtc datachannel timeout => switched to Failed");
                    self.state = State::Failed;
                    self.outputs.push_back(Out::TransportError(TransportError::ConnectError(ConnectErrorReason::Timeout)));
                }
            }
            State::Reconnecting { datachannel: _, at_ms } => {
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
            Str0mInput::IceConnectionStateChange(ice) => match (&self.state, ice) {
                (State::Connected { datachannel: dc, at_ms: _ }, IceConnectionState::Disconnected) => {
                    self.state = State::Reconnecting { datachannel: *dc, at_ms: now_ms };
                    self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnecting)));
                }
                (State::Reconnecting { datachannel: dc, at_ms: _ }, IceConnectionState::Completed) => {
                    self.state = State::Connected { datachannel: *dc, at_ms: now_ms };
                    self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnected)));
                }
                (State::Reconnecting { datachannel: dc, at_ms: _ }, IceConnectionState::Connected) => {
                    self.state = State::Connected { datachannel: *dc, at_ms: now_ms };
                    self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Reconnected)));
                }
                _ => {}
            },
            Str0mInput::Connected => {
                self.state = State::Connected { datachannel: false, at_ms: now_ms };
                log::info!("[SdkTransportLifeCycle] on webrtc connected => switched to {:?}", self.state);
            }
            Str0mInput::ChannelOpen(_, _) => match &self.state {
                State::Connected { datachannel, at_ms } => {
                    self.state = State::Connected { datachannel: true, at_ms: now_ms };
                    self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Connected)));
                }
                _ => {}
            },
            Str0mInput::ChannelClosed(_) => {
                self.state = State::Closed;
                self.outputs.push_back(Out::ToEndpoint(TransIn::State(TransportStateEvent::Disconnected)));
            }
            _ => {}
        }
    }

    fn on_endpoint_event(&mut self, now_ms: u64, event: &TransportOutgoingEvent<endpoint::EndpointRpcOut, endpoint::rpc::RemoteTrackRpcOut, endpoint::rpc::LocalTrackRpcOut>) {}

    fn pop_action(&mut self) -> Option<Out> {
        self.outputs.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use str0m::IceConnectionState;
    use transport::{ConnectErrorReason, ConnectionErrorReason, TransportError, TransportIncomingEvent as TransIn, TransportStateEvent};

    use crate::{
        transport::{
            internal::Str0mInput,
            life_cycle::{
                sdk::{CONNECT_TIMEOUT, RECONNECT_TIMEOUT},
                TransportLifeCycleAction as Out,
            },
        },
        TransportLifeCycle,
    };

    use super::SdkTransportLifeCycle;

    #[test]
    fn simple() {
        let mut life_cycle = SdkTransportLifeCycle::new(0);

        // webrtc connected should not switch
        life_cycle.on_transport_event(0, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), None);

        // next datachannel connected should switch to connected
        life_cycle.on_transport_event(0, &Str0mInput::ChannelOpen(0, "data".to_string()));
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

        // next datachannel disconnect should switch to closed
        life_cycle.on_transport_event(0, &Str0mInput::ChannelClosed(0));
        assert_eq!(life_cycle.pop_action(), Some(Out::ToEndpoint(TransIn::State(TransportStateEvent::Disconnected))));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn connect_timeout() {
        let mut life_cycle = SdkTransportLifeCycle::new(0);

        life_cycle.on_tick(CONNECT_TIMEOUT - 1);
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(CONNECT_TIMEOUT);
        assert_eq!(life_cycle.pop_action(), Some(Out::TransportError(TransportError::ConnectError(ConnectErrorReason::Timeout))));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn connect_datachannel_timeout() {
        let mut life_cycle = SdkTransportLifeCycle::new(0);

        // webrtc connected should not switch
        life_cycle.on_transport_event(1000, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(1000 + CONNECT_TIMEOUT - 1);
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(1000 + CONNECT_TIMEOUT);
        assert_eq!(life_cycle.pop_action(), Some(Out::TransportError(TransportError::ConnectError(ConnectErrorReason::Timeout))));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn reconnect_timeout() {
        let mut life_cycle = SdkTransportLifeCycle::new(0);

        // webrtc connected should not switch
        life_cycle.on_transport_event(100, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), None);

        // next datachannel connected should switch to connected
        life_cycle.on_transport_event(200, &Str0mInput::ChannelOpen(0, "data".to_string()));
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
}
