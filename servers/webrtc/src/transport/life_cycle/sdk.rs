use str0m::IceConnectionState;
use transport::MediaIncomingEvent;

use super::TransportLifeCycle;

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
    fn on_webrtc_connected(&mut self) -> MediaIncomingEvent {
        self.state = State::Connected { datachannel: false };
        log::info!("[SdkTransportLifeCycle] on webrtc connected => switched to {:?}", self.state);
        MediaIncomingEvent::Continue
    }

    fn on_ice_state(&mut self, ice: IceConnectionState) -> MediaIncomingEvent {
        let res = match (&self.state, ice) {
            (State::Connected { datachannel: dc }, IceConnectionState::Disconnected) => {
                self.state = State::Reconnecting { datachannel: *dc };
                MediaIncomingEvent::Reconnecting
            }
            (State::Reconnecting { datachannel: dc }, IceConnectionState::Completed) => {
                self.state = State::Connected { datachannel: *dc };
                MediaIncomingEvent::Reconnected
            }
            (State::Reconnecting { datachannel: dc }, IceConnectionState::Connected) => {
                self.state = State::Connected { datachannel: *dc };
                MediaIncomingEvent::Reconnected
            }
            _ => MediaIncomingEvent::Continue,
        };
        log::info!("[SdkTransportLifeCycle] on ice state {:?} => switched to {:?}", ice, self.state);
        res
    }

    fn on_data_channel(&mut self, connected: bool) -> MediaIncomingEvent {
        let res = match (connected, &self.state) {
            (true, State::Connected { datachannel: false }) => {
                self.state = State::Connected { datachannel: true };
                MediaIncomingEvent::Connected
            }
            (false, _) => {
                self.state = State::Closed;
                MediaIncomingEvent::Disconnected
            }
            _ => MediaIncomingEvent::Continue,
        };
        log::info!("[SdkTransportLifeCycle] on datachannel connected {} => switched to {:?}", connected, self.state);
        res
    }
}
