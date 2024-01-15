use transport::{TransportError, TransportStateEvent};

use super::internal::Str0mInput;

pub mod datachannel;
pub mod no_datachannel;

pub trait TransportLifeCycle: Send {
    fn on_tick(&mut self, now_ms: u64);
    fn on_transport_event(&mut self, now_ms: u64, event: &Str0mInput);
    fn pop_action(&mut self) -> Option<Result<TransportStateEvent, TransportError>>;
}
