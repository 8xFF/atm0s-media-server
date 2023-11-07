use std::net::SocketAddr;

use super::{sip_request::SipRequest, sip_response::SipResponse};

pub mod call_in;
pub mod call_out;
pub mod register;

pub enum ProcessorError {
    Timeout,
    WrongMessage,
    WrongState,
}

pub enum ProcessorAction<C> {
    Finished(Result<(), String>),
    SendRequest(Option<SocketAddr>, SipRequest),
    SendResponse(Option<SocketAddr>, SipResponse),
    LogicOutput(C),
}

pub trait Processor<C> {
    fn start(&mut self, now_ms: u64) -> Result<(), ProcessorError>;
    fn on_tick(&mut self, now_ms: u64) -> Result<(), ProcessorError>;
    fn on_req(&mut self, now_ms: u64, req: SipRequest) -> Result<(), ProcessorError>;
    fn on_res(&mut self, now_ms: u64, res: SipResponse) -> Result<(), ProcessorError>;
    fn pop_action(&mut self) -> Option<ProcessorAction<C>>;
}
