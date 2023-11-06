use std::{collections::HashMap, net::SocketAddr};

use rsip::{headers::CallId, Method};

use crate::processor::Processor;

use self::{
    processor::{register::RegisterProcessor, ProcessorAction},
    sip_request::SipRequest,
    sip_response::SipResponse,
};

pub mod processor;
pub mod sip_request;
pub mod sip_response;

pub type GroupId = (SocketAddr, CallId);

pub enum SipServerError {}

#[derive(Debug)]
pub enum SipServerEvent {
    OnRegisterValidate(GroupId, String),
    OnInCallStarted(GroupId, SipRequest),
    OnInCallRequest(GroupId, SipRequest),
    OnInCallEnded(GroupId),
    SendRes(SocketAddr, SipResponse),
    SendReq(SocketAddr, SipRequest),
}

pub struct SipServer {
    groups: HashMap<GroupId, u64>,
    register_processors: HashMap<GroupId, RegisterProcessor>,
    actions: Vec<SipServerEvent>,
}

impl SipServer {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            register_processors: HashMap::new(),
            actions: Vec::new(),
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {}

    pub fn reply_register_validate(&mut self, group_id: GroupId, accept: bool) {
        if let Some(processor) = self.register_processors.get_mut(&group_id) {
            processor.accept(accept);
            self.process_register_processor(&group_id);
        }
    }

    pub fn on_req(&mut self, now_ms: u64, from: SocketAddr, req: SipRequest) -> Result<(), SipServerError> {
        match req.method() {
            Method::Register => {
                let group_id: (SocketAddr, CallId) = (from, req.call_id.clone());
                match self.register_processors.entry(group_id.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        entry.get_mut().on_req(now_ms, req);
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let mut processor = RegisterProcessor::new(now_ms, req);
                        processor.start(now_ms);
                        entry.insert(processor);
                    }
                };
                self.process_register_processor(&group_id);
                Ok(())
            }
            Method::Invite => {
                todo!()
            }
            Method::Options => {
                todo!()
            }
            _ => {
                todo!()
            }
        }
    }

    pub fn on_res(&mut self, now_ms: u64, from: SocketAddr, res: SipResponse) -> Result<(), SipServerError> {
        todo!()
        // TODO check type of response
        // Finding transaction for that response and send it to transaction
    }

    pub fn pop_action(&mut self) -> Option<SipServerEvent> {
        self.actions.pop()
    }

    fn process_register_processor(&mut self, group_id: &(SocketAddr, CallId)) -> Option<()> {
        let mut processor = self.register_processors.get_mut(group_id)?;
        while let Some(action) = processor.pop_action() {
            match action {
                ProcessorAction::Finished(res) => {
                    self.register_processors.remove(group_id);
                    break;
                }
                ProcessorAction::SendRequest(req) => {
                    self.actions.push(SipServerEvent::SendReq(group_id.0, req));
                }
                ProcessorAction::SendResponse(res) => {
                    self.actions.push(SipServerEvent::SendRes(group_id.0, res));
                }
                ProcessorAction::LogicOutput(action) => match action {
                    processor::register::RegisterProcessorAction::Validate(username) => {
                        self.actions.push(SipServerEvent::OnRegisterValidate(group_id.clone(), username));
                    }
                },
            }
        }
        Some(())
    }
}
