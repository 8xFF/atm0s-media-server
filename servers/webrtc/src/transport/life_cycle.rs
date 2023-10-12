use str0m::IceConnectionState;
use transport::MediaIncomingEvent;

pub(crate) mod whip;

pub trait TransportLifeCycle: Send {
    fn on_webrtc_connected(&mut self) -> MediaIncomingEvent;
    fn on_ice_state(&mut self, ice: IceConnectionState) -> MediaIncomingEvent;
    fn on_data_channel(&mut self, connected: bool) -> MediaIncomingEvent;
}
