use str0m::IceConnectionState;
use transport::MediaIncomingEvent;

use super::TransportLifeCycle;

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
    fn on_webrtc_connected(&mut self) -> MediaIncomingEvent {
        self.state = State::Connected;
        log::info!("[WhipTransportLifeCycle] on webrtc connected => switched to {:?}", self.state);
        MediaIncomingEvent::Connected
    }

    fn on_ice_state(&mut self, ice: IceConnectionState) -> MediaIncomingEvent {
        let res = match (&self.state, ice) {
            (State::Connected, IceConnectionState::Disconnected) => {
                self.state = State::Reconnecting;
                MediaIncomingEvent::Reconnecting
            }
            (State::Reconnecting, IceConnectionState::Completed) => {
                self.state = State::Connected;
                MediaIncomingEvent::Reconnected
            }
            (State::Reconnecting, IceConnectionState::Connected) => {
                self.state = State::Connected;
                MediaIncomingEvent::Reconnected
            }
            _ => MediaIncomingEvent::Continue,
        };
        log::info!("[WhipTransportLifeCycle] on ice state {:?} => switched to {:?}", ice, self.state);
        res
    }

    fn on_data_channel(&mut self, connected: bool) -> MediaIncomingEvent {
        panic!("should not happend")
    }
}
