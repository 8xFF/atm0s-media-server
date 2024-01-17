use std::{collections::HashMap, fmt::Display, net::SocketAddr};

use bytes::Bytes;
use rsip::{
    headers::{CallId, UntypedHeader},
    Method,
};

use crate::processor::Processor;

use self::{
    processor::{register::RegisterProcessor, ProcessorAction, ProcessorError},
    sip_request::SipRequest,
    sip_response::SipResponse,
};

mod data;
pub mod processor;
pub mod sip_request;
pub mod sip_response;
mod transaction;
mod utils;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct GroupId(SocketAddr, String);

impl GroupId {
    pub fn from_raw(from: SocketAddr, call_id: &CallId) -> Self {
        Self(from, call_id.value().to_string())
    }

    pub fn addr(&self) -> SocketAddr {
        self.0
    }

    pub fn call_id(&self) -> &str {
        &self.1
    }
}

impl Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.0, self.1)
    }
}

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

#[derive(Debug)]
pub enum SipServerError {
    ProcessorError(ProcessorError),
    ProcessorNotFound,
}

#[derive(Debug)]
pub enum SipServerEvent {
    OnRegisterValidate(GroupId, String, String, String, String, String),
    OnInCallStarted(GroupId, SipRequest),
    OnInCallRequest(GroupId, SipRequest),
    OnInCallResponse(GroupId, SipResponse),
    OnOutCallRequest(GroupId, SipRequest),
    OnOutCallResponse(GroupId, SipResponse),
    SendRes(SocketAddr, SipResponse),
    SendReq(SocketAddr, SipRequest),
}

pub struct SipCore {
    register_processors: HashMap<GroupId, RegisterProcessor>,
    invite_in_groups: HashMap<GroupId, ()>,
    invite_out_groups: HashMap<GroupId, ()>,
    actions: Vec<SipServerEvent>,
}

impl SipCore {
    pub fn new() -> Self {
        Self {
            register_processors: HashMap::new(),
            invite_in_groups: HashMap::new(),
            invite_out_groups: HashMap::new(),
            actions: Vec::new(),
        }
    }

    pub fn on_tick(&mut self, _now_ms: u64) {}

    pub fn reply_register_validate(&mut self, group_id: &GroupId, accept: bool) {
        if let Some(processor) = self.register_processors.get_mut(group_id) {
            processor.accept(accept);
            self.process_register_processor(group_id);
        }
    }

    pub fn open_out_call(&mut self, group_id: &GroupId) {
        log::info!("create out call {:?}", group_id);
        self.invite_out_groups.insert(group_id.clone(), ());
    }

    pub fn close_out_call(&mut self, group_id: &GroupId) {
        self.invite_out_groups.remove(group_id);
    }

    pub fn close_in_call(&mut self, group_id: &GroupId) {
        self.invite_in_groups.remove(group_id);
    }

    pub fn on_req(&mut self, now_ms: u64, from: SocketAddr, req: SipRequest) -> Result<(), SipServerError> {
        match req.method() {
            Method::Register => {
                let group_id = GroupId(from, req.call_id.clone().into());
                match self.register_processors.entry(group_id.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        entry.get_mut().on_req(now_ms, req).map_err(|e| SipServerError::ProcessorError(e))?;
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let mut processor = RegisterProcessor::new(now_ms, req);
                        processor.start(now_ms).map_err(|e| SipServerError::ProcessorError(e))?;
                        entry.insert(processor);
                    }
                };
                self.process_register_processor(&group_id);
                Ok(())
            }
            Method::Invite => {
                let group_id = GroupId(from, req.call_id.clone().into());
                if let Some(_) = self.invite_in_groups.get(&group_id) {
                    self.actions.push(SipServerEvent::OnInCallRequest(group_id, req));
                    Ok(())
                } else {
                    self.invite_in_groups.insert(group_id.clone(), ());
                    self.actions.push(SipServerEvent::OnInCallStarted(group_id, req));
                    Ok(())
                }
            }
            _ => {
                let group_id = GroupId(from, req.call_id.clone().into());
                if let Some(_) = self.invite_in_groups.get(&group_id) {
                    self.actions.push(SipServerEvent::OnInCallRequest(group_id, req));
                    Ok(())
                } else if let Some(_) = self.invite_out_groups.get(&group_id) {
                    self.actions.push(SipServerEvent::OnOutCallRequest(group_id, req));
                    Ok(())
                } else {
                    log::info!("on_req not found {:?}, {:?}", group_id, self.invite_out_groups);
                    Err(SipServerError::ProcessorNotFound)
                }
            }
        }
    }

    pub fn on_res(&mut self, _now_ms: u64, from: SocketAddr, res: SipResponse) -> Result<(), SipServerError> {
        let group_id = GroupId(from, res.call_id.clone().into());
        if let Some(_) = self.invite_in_groups.get(&group_id) {
            self.actions.push(SipServerEvent::OnInCallResponse(group_id, res));
            Ok(())
        } else if let Some(_) = self.invite_out_groups.get(&group_id) {
            self.actions.push(SipServerEvent::OnOutCallResponse(group_id, res));
            Ok(())
        } else {
            log::info!("on_res not found {:?}, {:?}", group_id, self.invite_out_groups);
            Err(SipServerError::ProcessorNotFound)
        }
    }

    pub fn pop_action(&mut self) -> Option<SipServerEvent> {
        self.actions.pop()
    }

    fn process_register_processor(&mut self, group_id: &GroupId) -> Option<()> {
        let processor = self.register_processors.get_mut(group_id)?;
        while let Some(action) = processor.pop_action() {
            match action {
                ProcessorAction::Finished(_) => {
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
                    processor::register::RegisterProcessorAction::Validate(digest, nonce, username, realm, hashed_password) => {
                        self.actions.push(SipServerEvent::OnRegisterValidate(group_id.clone(), digest, nonce, username, realm, hashed_password));
                    }
                },
            }
        }
        Some(())
    }
}
