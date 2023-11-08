use std::{collections::HashMap, fmt::Display, net::SocketAddr};

use bytes::Bytes;
use rsip::{headers::CallId, Method};

use crate::processor::Processor;

use self::{
    processor::{register::RegisterProcessor, ProcessorAction},
    sip_request::SipRequest,
    sip_response::SipResponse,
};

mod data;
pub mod processor;
pub mod sip_request;
pub mod sip_response;
mod transaction;
mod utils;

pub type GroupId = (SocketAddr, CallId);

pub enum SipMessage {
    Request(SipRequest),
    Response(SipResponse),
}

impl SipMessage {
    pub fn to_bytes(self) -> Bytes {
        match self {
            SipMessage::Request(req) => req.to_bytes(),
            SipMessage::Response(res) => res.to_bytes(),
        }
    }
}

impl Display for SipMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SipMessage::Request(req) => write!(f, "Req({})", req.method()),
            SipMessage::Response(res) => write!(f, "Res({})", res.raw.status_code()),
        }
    }
}

pub enum SipServerError {}

#[derive(Debug)]
pub enum SipServerEvent {
    OnRegisterValidate(GroupId, String),
    OnInCallStarted(GroupId, SipRequest),
    OnInCallRequest(GroupId, SipRequest),
    OnInCallResponse(GroupId, SipResponse),
    SendRes(SocketAddr, SipResponse),
    SendReq(SocketAddr, SipRequest),
}

pub struct SipServer {
    register_processors: HashMap<GroupId, RegisterProcessor>,
    invite_groups: HashMap<GroupId, ()>,
    actions: Vec<SipServerEvent>,
}

impl SipServer {
    pub fn new() -> Self {
        Self {
            register_processors: HashMap::new(),
            invite_groups: HashMap::new(),
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

    pub fn close_in_call(&mut self, group_id: &GroupId) {
        self.invite_groups.remove(group_id);
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
                let group_id: (SocketAddr, CallId) = (from, req.call_id.clone());
                if let Some(_) = self.invite_groups.get(&group_id) {
                    self.actions.push(SipServerEvent::OnInCallRequest(group_id, req));
                    Ok(())
                } else {
                    self.invite_groups.insert(group_id.clone(), ());
                    self.actions.push(SipServerEvent::OnInCallStarted(group_id, req));
                    Ok(())
                }
            }
            _ => {
                let group_id: (SocketAddr, CallId) = (from, req.call_id.clone());
                if let Some(_) = self.invite_groups.get(&group_id) {
                    self.actions.push(SipServerEvent::OnInCallRequest(group_id, req));
                    Ok(())
                } else {
                    //TODO handle this
                    Ok(())
                }
            }
        }
    }

    pub fn on_res(&mut self, now_ms: u64, from: SocketAddr, res: SipResponse) -> Result<(), SipServerError> {
        let group_id: (SocketAddr, CallId) = (from, res.call_id.clone());
        if let Some(_) = self.invite_groups.get(&group_id) {
            self.actions.push(SipServerEvent::OnInCallResponse(group_id, res));
            Ok(())
        } else {
            //TODO handle this
            Ok(())
        }
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
                ProcessorAction::SendRequest(remote_addr, req) => {
                    self.actions.push(SipServerEvent::SendReq(remote_addr.unwrap_or(group_id.0), req));
                }
                ProcessorAction::SendResponse(remote_addr, res) => {
                    self.actions.push(SipServerEvent::SendRes(remote_addr.unwrap_or(group_id.0), res));
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
