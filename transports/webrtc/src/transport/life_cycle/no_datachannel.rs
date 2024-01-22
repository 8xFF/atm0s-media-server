use std::collections::VecDeque;

use str0m::IceConnectionState;
use transport::{ConnectErrorReason, ConnectionErrorReason, TransportError, TransportStateEvent};

use crate::transport::internal::Str0mInput;

use super::TransportLifeCycle;

const CONNECT_TIMEOUT: u64 = 10000;
const RECONNECT_TIMEOUT: u64 = 30000;

#[derive(Debug)]
enum State {
    New { at_ms: u64 },
    Connected,
    Reconnecting { at_ms: u64 },
    Failed,
}

pub struct TransportNoDatachannelLifeCycle {
    state: State,
    outputs: VecDeque<Result<TransportStateEvent, TransportError>>,
}

impl TransportNoDatachannelLifeCycle {
    pub fn new(now_ms: u64) -> Self {
        log::info!("[TransportNoDatachannelLifeCycle] new");
        Self {
            state: State::New { at_ms: now_ms },
            outputs: VecDeque::new(),
        }
    }
}

impl TransportLifeCycle for TransportNoDatachannelLifeCycle {
    fn on_tick(&mut self, now_ms: u64) {
        match &self.state {
            State::New { at_ms } => {
                if at_ms + CONNECT_TIMEOUT <= now_ms {
                    log::info!("[TransportNoDatachannelLifeCycle] on webrtc connect timeout => switched to Failed");
                    self.state = State::Failed;
                    self.outputs.push_back(Err(TransportError::ConnectError(ConnectErrorReason::Timeout)));
                }
            }
            State::Reconnecting { at_ms } => {
                if at_ms + RECONNECT_TIMEOUT <= now_ms {
                    log::info!("[TransportNoDatachannelLifeCycle] on webrtc reconnect timeout => switched to Failed");
                    self.state = State::Failed;
                    self.outputs.push_back(Err(TransportError::ConnectionError(ConnectionErrorReason::Timeout)));
                }
            }
            _ => {}
        }
    }

    fn on_transport_event(&mut self, now_ms: u64, event: &Str0mInput) {
        match event {
            Str0mInput::Connected => {
                self.state = State::Connected;
                log::info!("[TransportNoDatachannelLifeCycle] on webrtc connected => switched to {:?}", self.state);
                self.outputs.push_back(Ok(TransportStateEvent::Connected));
            }
            Str0mInput::IceConnectionStateChange(ice) => match (&self.state, ice) {
                (State::Connected, IceConnectionState::Disconnected) => {
                    self.state = State::Reconnecting { at_ms: now_ms };
                    self.outputs.push_back(Ok(TransportStateEvent::Reconnecting));
                }
                (State::Reconnecting { .. }, IceConnectionState::Completed) => {
                    self.state = State::Connected;
                    self.outputs.push_back(Ok(TransportStateEvent::Reconnected));
                }
                (State::Reconnecting { .. }, IceConnectionState::Connected) => {
                    self.state = State::Connected;
                    self.outputs.push_back(Ok(TransportStateEvent::Reconnected));
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn pop_action(&mut self) -> Option<Result<TransportStateEvent, TransportError>> {
        self.outputs.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use str0m::IceConnectionState;
    use transport::{ConnectErrorReason, ConnectionErrorReason, TransportError, TransportStateEvent};

    #[test]
    fn simple() {
        let mut life_cycle = TransportNoDatachannelLifeCycle::new(0);

        // webrtc connected should fire connected
        life_cycle.on_transport_event(0, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), Some(Ok(TransportStateEvent::Connected)));
        assert_eq!(life_cycle.pop_action(), None);

        // next ice disconnect should switch to reconnecting
        life_cycle.on_transport_event(0, &Str0mInput::IceConnectionStateChange(IceConnectionState::Disconnected));
        assert_eq!(life_cycle.pop_action(), Some(Ok(TransportStateEvent::Reconnecting)));
        assert_eq!(life_cycle.pop_action(), None);

        // next connected should switch to reconnected
        life_cycle.on_transport_event(0, &Str0mInput::IceConnectionStateChange(IceConnectionState::Connected));
        assert_eq!(life_cycle.pop_action(), Some(Ok(TransportStateEvent::Reconnected)));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn connect_timeout() {
        let mut life_cycle = TransportNoDatachannelLifeCycle::new(0);

        life_cycle.on_tick(CONNECT_TIMEOUT - 1);
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(CONNECT_TIMEOUT);
        assert_eq!(life_cycle.pop_action(), Some(Err(TransportError::ConnectError(ConnectErrorReason::Timeout))));
        assert_eq!(life_cycle.pop_action(), None);
    }

    #[test]
    fn reconnect_timeout() {
        let mut life_cycle = TransportNoDatachannelLifeCycle::new(0);

        // webrtc connected should not switch
        life_cycle.on_transport_event(100, &Str0mInput::Connected);
        assert_eq!(life_cycle.pop_action(), Some(Ok(TransportStateEvent::Connected)));
        assert_eq!(life_cycle.pop_action(), None);

        // next ice disconnect should switch to reconnecting
        life_cycle.on_transport_event(1000, &Str0mInput::IceConnectionStateChange(IceConnectionState::Disconnected));
        assert_eq!(life_cycle.pop_action(), Some(Ok(TransportStateEvent::Reconnecting)));
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(1000 + RECONNECT_TIMEOUT - 1);
        assert_eq!(life_cycle.pop_action(), None);

        life_cycle.on_tick(1000 + RECONNECT_TIMEOUT);
        assert_eq!(life_cycle.pop_action(), Some(Err(TransportError::ConnectionError(ConnectionErrorReason::Timeout))));
        assert_eq!(life_cycle.pop_action(), None);
    }
}
