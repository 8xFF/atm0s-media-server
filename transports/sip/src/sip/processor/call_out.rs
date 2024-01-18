use std::{collections::VecDeque, net::SocketAddr};

use rsip::{
    headers::{self, CallId, ContentType},
    typed::{self, Contact, From, To},
    Headers, Method, Scheme, StatusCode,
};

use crate::{
    sip::{
        transaction::client_invite_transaction::{ClientInviteTransaction, ClientInviteTransactionAction, ClientInviteTransactionEvent, Terminated},
        utils::generate_random_string,
    },
    sip_request::SipRequest,
    sip_response::SipResponse,
};

use super::{Processor, ProcessorAction, ProcessorError};

const T1: u64 = 500;
const T2: u64 = 500 * 64;

enum State {
    Connecting { transaction: ClientInviteTransaction, canceling: bool },
    InCall {},
    Bye { timer_resend_res: u64, timeout: u64 },
    End,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CallOutProcessorAction {
    Accepted(Option<(ContentType, Vec<u8>)>),
}

#[allow(unused)]
pub struct CallOutProcessor {
    state: State,
    local_contact: Contact,
    call_id: CallId,
    local_from: From,
    remote_to: To,
    remote_contact_addr: Option<SocketAddr>,
    actions: VecDeque<ProcessorAction<CallOutProcessorAction>>,
}

impl CallOutProcessor {
    pub fn new(now_ms: u64, local_contact: Contact, call_id: CallId, local_from: From, remote_to: To, sdp: &str) -> Self {
        Self {
            state: State::Connecting {
                transaction: ClientInviteTransaction::new(now_ms, call_id.clone(), local_contact.clone(), local_from.clone(), remote_to.clone(), sdp),
                canceling: false,
            },
            local_contact,
            call_id,
            local_from,
            remote_to,
            remote_contact_addr: None,
            actions: VecDeque::new(),
        }
    }

    pub fn cancel(&mut self, _now_ms: u64) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::Connecting { transaction: _, canceling } => {
                *canceling = true;
                let req = self.create_request(Method::Cancel, typed::CSeq { seq: 2, method: Method::Cancel }.into());
                self.actions.push_back(ProcessorAction::SendRequest(self.remote_contact_addr, req));
                Ok(())
            }
            _ => Err(super::ProcessorError::WrongState),
        }
    }

    pub fn end(&mut self, now_ms: u64) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::InCall { .. } => {
                let req = self.create_request(Method::Bye, typed::CSeq { seq: 2, method: Method::Bye }.into());
                self.actions.push_back(ProcessorAction::SendRequest(self.remote_contact_addr, req));
                self.state = State::Bye {
                    timer_resend_res: now_ms + T1,
                    timeout: now_ms + T2,
                };
                Ok(())
            }
            _ => Err(super::ProcessorError::WrongState),
        }
    }

    /// Auto determine the state and send CANCEL or BYE
    pub fn close(&mut self, now_ms: u64) {
        match &self.state {
            State::Connecting { .. } => {
                self.cancel(now_ms).expect("Should ok");
            }
            State::InCall { .. } => {
                self.end(now_ms).expect("Should ok");
            }
            State::Bye { .. } => {}
            State::End => {}
        }
    }

    fn process_transaction(&mut self, _now_ms: u64) {
        let transaction = match &mut self.state {
            State::Connecting { transaction, .. } => transaction,
            _ => return,
        };

        while let Some(action) = transaction.pop_action() {
            match action {
                ClientInviteTransactionAction::Req(addr, req) => {
                    self.actions.push_back(ProcessorAction::SendRequest(addr, req));
                }
                ClientInviteTransactionAction::Terminated(Terminated::Accepted(body)) => {
                    self.actions.push_back(ProcessorAction::LogicOutput(CallOutProcessorAction::Accepted(body)));
                    self.state = State::InCall {};
                    break;
                }
                ClientInviteTransactionAction::Terminated(Terminated::Rejected) => {
                    self.state = State::End;
                    self.actions.push_back(ProcessorAction::Finished(Ok(())));
                    break;
                }
                ClientInviteTransactionAction::Terminated(Terminated::Timeout) => {
                    self.state = State::End;
                    self.actions.push_back(ProcessorAction::Finished(Err("TIMEOUT".to_string())));
                    break;
                }
            }
        }
    }

    fn create_request(&self, method: Method, cseq: headers::CSeq) -> SipRequest {
        let request = rsip::Request {
            method,
            uri: rsip::Uri {
                scheme: Some(Scheme::Sip),
                auth: None,
                host_with_port: self.remote_to.uri.host_with_port.clone(),
                params: vec![],
                headers: vec![],
            },
            version: rsip::Version::V2,
            headers: Headers::from(vec![
                rsip::Header::Via(headers::Via::from(format!(
                    "SIP/2.0/UDP {};branch=z9hG4bK-{}",
                    self.local_contact.uri.host_with_port,
                    generate_random_string(8)
                ))),
                rsip::Header::MaxForwards(headers::MaxForwards::from(70)),
                rsip::Header::From(self.local_from.clone().into()),
                rsip::Header::To(self.remote_to.clone().into()),
                rsip::Header::CallId(self.call_id.clone()),
                rsip::Header::CSeq(cseq),
                rsip::Header::Contact(self.local_contact.clone().into()),
                rsip::Header::UserAgent(headers::UserAgent::from("8xff-sip-media-server")),
                rsip::Header::ContentLength(headers::ContentLength::from(0)),
            ]),
            body: vec![],
        };
        SipRequest::from(request).expect("Should be valid request")
    }
}

impl Processor<CallOutProcessorAction> for CallOutProcessor {
    fn start(&mut self, _now_ms: u64) -> Result<(), ProcessorError> {
        Ok(())
    }

    fn on_tick(&mut self, now_ms: u64) -> Result<(), ProcessorError> {
        match &mut self.state {
            State::Connecting { transaction, .. } => {
                transaction.on_event(now_ms, ClientInviteTransactionEvent::Timer);
                self.process_transaction(now_ms);
            }
            State::InCall {} => {}
            State::Bye { timer_resend_res, timeout } => {
                if now_ms >= *timeout {
                    self.state = State::End;
                    //TODO avoid texting error
                    self.actions.push_back(ProcessorAction::Finished(Err("TIMEOUT".to_string())));
                } else if now_ms >= *timer_resend_res {
                    log::warn!("[CallInProcessor] resend BYE");
                    *timer_resend_res = now_ms + T1;
                    let req = self.create_request(Method::Bye, typed::CSeq { seq: 2, method: Method::Bye }.into());
                    self.actions.push_back(ProcessorAction::SendRequest(self.remote_contact_addr, req));
                }
            }
            State::End => {}
        }
        Ok(())
    }
    fn on_req(&mut self, _now_ms: u64, req: SipRequest) -> Result<(), ProcessorError> {
        log::info!("on req {:?}", req);
        match &mut self.state {
            State::Connecting { .. } => {}
            State::InCall {} => match req.method() {
                Method::Bye => {
                    let res = req.build_response(StatusCode::OK, None);
                    self.actions.push_back(ProcessorAction::SendResponse(self.remote_contact_addr, res));
                    self.actions.push_back(ProcessorAction::Finished(Ok(())));
                    self.state = State::End;
                }
                _ => {}
            },
            State::Bye { .. } => {}
            State::End => {}
        }
        Ok(())
    }

    fn on_res(&mut self, now_ms: u64, res: SipResponse) -> Result<(), ProcessorError> {
        if let Some(contact) = res.header_contact() {
            if let Some(addr) = contact.uri.host_with_port.try_into().ok() {
                self.remote_contact_addr = Some(addr);
                log::info!("[CallInProcessor] update sip dest to {}", addr);
            }
        }
        match &mut self.state {
            State::Connecting { transaction, .. } => {
                transaction.on_event(now_ms, ClientInviteTransactionEvent::Res(res));
                self.process_transaction(now_ms);
            }
            State::Bye { .. } => {
                if res.cseq.method == Method::Bye {
                    self.state = State::End;
                    self.actions.push_back(ProcessorAction::Finished(Ok(())));
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn pop_action(&mut self) -> Option<ProcessorAction<CallOutProcessorAction>> {
        self.actions.pop_front()
    }
}

//TODO test same with call_in
