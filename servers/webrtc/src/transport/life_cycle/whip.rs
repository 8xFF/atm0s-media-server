use str0m::IceConnectionState;

use super::{TransportLifeCycle, TransportLifeCycleEvent};

#[derive(Debug)]
pub enum State {
    New,
    Connected,
    Reconnecting,
    Failed,
    Closed,
}

pub struct WhipTransportLifeCycle {
    state: State,
}

impl WhipTransportLifeCycle {
    pub fn new() -> Self {
        log::info!("[WhipTransportLifeCycle] new");
        Self { state: State::New }
    }
}

impl TransportLifeCycle for WhipTransportLifeCycle {
    fn on_tick(&mut self, now_ms: u64) -> Option<TransportLifeCycleEvent> {
        None
    }

    fn on_webrtc_connected(&mut self) -> Option<TransportLifeCycleEvent> {
        self.state = State::Connected;
        log::info!("[WhipTransportLifeCycle] on webrtc connected => switched to {:?}", self.state);
        Some(TransportLifeCycleEvent::Connected)
    }

    fn on_ice_state(&mut self, ice: IceConnectionState) -> Option<TransportLifeCycleEvent> {
        let res = match (&self.state, ice) {
            (State::Connected, IceConnectionState::Disconnected) => {
                self.state = State::Reconnecting;
                Some(TransportLifeCycleEvent::Reconnecting)
            }
            (State::Reconnecting, IceConnectionState::Completed) => {
                self.state = State::Connected;
                Some(TransportLifeCycleEvent::Reconnected)
            }
            (State::Reconnecting, IceConnectionState::Connected) => {
                self.state = State::Connected;
                Some(TransportLifeCycleEvent::Reconnected)
            }
            _ => None,
        };

        if res.is_some() {
            log::info!("[WhipTransportLifeCycle] on ice state {:?} => switched to {:?}", ice, self.state);
        }
        res
    }

    fn on_data_channel(&mut self, connected: bool) -> Option<TransportLifeCycleEvent> {
        panic!("should not happend")
    }
}

//TODO test this
