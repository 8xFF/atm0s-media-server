use str0m::IceConnectionState;
use transport::MediaIncomingEvent;

pub enum State {
    New,
    Connected { datachannel: bool },
    Reconnecting { datachannel: bool },
    Failed,
    Closed,
}

pub struct WebrtcLifeCycle {
    state: State,
}

impl WebrtcLifeCycle {
    pub fn new() -> Self {
        Self { state: State::New }
    }

    pub fn on_webrtc_connected(&mut self) -> MediaIncomingEvent {
        self.state = State::Connected { datachannel: false };
        MediaIncomingEvent::Continue
    }

    pub fn on_ice_state(&mut self, ice: IceConnectionState) -> MediaIncomingEvent {
        match (&self.state, ice) {
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
        }
    }

    pub fn on_data_channel(&mut self, connected: bool) -> MediaIncomingEvent {
        match (connected, &self.state) {
            (true, State::Connected { datachannel: false }) => {
                self.state = State::Connected { datachannel: true };
                MediaIncomingEvent::Connected
            }
            (false, _) => {
                self.state = State::Closed;
                MediaIncomingEvent::Disconnected
            }
            _ => MediaIncomingEvent::Continue,
        }
    }
}
