use str0m::IceConnectionState;
use transport::MediaIncomingEvent;

use super::{TransportLifeCycle, TransportLifeCycleEvent};

#[derive(Debug)]
pub enum State {
    New,
    Connected { datachannel: bool },
    Reconnecting { datachannel: bool },
    Failed,
    Closed,
}

pub struct SdkTransportLifeCycle {
    state: State,
}

impl SdkTransportLifeCycle {
    pub fn new() -> Self {
        log::info!("[SdkTransportLifeCycle] new");
        Self { state: State::New }
    }
}

impl TransportLifeCycle for SdkTransportLifeCycle {
    fn on_tick(&mut self, now_ms: u64) -> Option<TransportLifeCycleEvent> {
        None
    }

    fn on_webrtc_connected(&mut self) -> Option<TransportLifeCycleEvent> {
        self.state = State::Connected { datachannel: false };
        log::info!("[SdkTransportLifeCycle] on webrtc connected => switched to {:?}", self.state);
        None
    }

    fn on_ice_state(&mut self, ice: IceConnectionState) -> Option<TransportLifeCycleEvent> {
        let res = match (&self.state, ice) {
            (State::Connected { datachannel: dc }, IceConnectionState::Disconnected) => {
                self.state = State::Reconnecting { datachannel: *dc };
                Some(TransportLifeCycleEvent::Reconnecting)
            }
            (State::Reconnecting { datachannel: dc }, IceConnectionState::Completed) => {
                self.state = State::Connected { datachannel: *dc };
                Some(TransportLifeCycleEvent::Reconnected)
            }
            (State::Reconnecting { datachannel: dc }, IceConnectionState::Connected) => {
                self.state = State::Connected { datachannel: *dc };
                Some(TransportLifeCycleEvent::Reconnected)
            }
            _ => None,
        };

        if res.is_some() {
            log::info!("[SdkTransportLifeCycle] on ice state {:?} => switched to {:?}", ice, self.state);
        }
        res
    }

    fn on_data_channel(&mut self, connected: bool) -> Option<TransportLifeCycleEvent> {
        let res = match (connected, &self.state) {
            (true, State::Connected { datachannel: false }) => {
                self.state = State::Connected { datachannel: true };
                Some(TransportLifeCycleEvent::Connected)
            }
            (false, _) => {
                self.state = State::Closed;
                Some(TransportLifeCycleEvent::Closed)
            }
            _ => None,
        };
        if res.is_some() {
            log::info!("[SdkTransportLifeCycle] on datachannel connected {} => switched to {:?}", connected, self.state);
        }
        res
    }
}

//TODO test this
