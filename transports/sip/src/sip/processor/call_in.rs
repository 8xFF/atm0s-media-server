use std::{collections::VecDeque, net::SocketAddr};

use rsip::{
    headers::{self, typed, ContentType},
    typed::Contact,
    Headers, HostWithPort, Method, Scheme, StatusCode,
};

use crate::{
    sip::{
        transaction::server_invite_transaction::{ServerInviteTransaction, ServerInviteTransactionAction, ServerInviteTransactionEvent, Terminated},
        utils::generate_random_string,
    },
    sip_request::SipRequest,
};

use super::{Processor, ProcessorAction};

/**
   Alice                     Bob
    |                        |
    |       INVITE F1        |
    |----------------------->|
    |    180 Ringing F2      |
    |<-----------------------|
    |                        |
    |       200 OK F3        |
    |<-----------------------|
    |         ACK F4         |
    |----------------------->|
    |   Both Way RTP Media   |
    |<======================>|
    |                        |
    |         BYE F5         |
    |<-----------------------|
    |       200 OK F6        |
    |----------------------->|
    |                        |
*/

const T1: u64 = 500;
const T2: u64 = 500 * 64;

enum State {
    Connecting { transaction: ServerInviteTransaction },
    InCall { timer_resend_res: Option<(u64, Option<(ContentType, Vec<u8>)>)> },
    Bye { timer_resend_res: u64, timeout: u64 },
    End,
}

pub enum CallInProcessorAction {}

pub struct CallInProcessor {
    state: State,
    local_contact: Contact,
    remote_contact_addr: Option<SocketAddr>,
    init_req: SipRequest,
    actions: VecDeque<ProcessorAction<CallInProcessorAction>>,
}

impl CallInProcessor {
    pub fn new(now_ms: u64, local_contact: Contact, req: SipRequest) -> Self {
        let remote_contact_addr = req.header_contact().map(|contact| contact.uri.host_with_port.try_into().ok()).flatten();
        if let Some(remote_contact_addr) = &remote_contact_addr {
            log::info!("[CallInProcessor] created with custom remote_contact_addr {}", remote_contact_addr);
        }
        Self {
            state: State::Connecting {
                transaction: ServerInviteTransaction::new(now_ms, local_contact.clone(), req.clone()),
            },
            local_contact,
            remote_contact_addr,
            init_req: req,
            actions: VecDeque::new(),
        }
    }

    pub fn ringing(&mut self, now_ms: u64) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::Connecting { transaction } => {
                log::info!("[CallInProcessor] switched ringing call");
                transaction.on_event(now_ms, ServerInviteTransactionEvent::Status(StatusCode::Ringing, None));
                Ok(())
            }
            _ => Err(super::ProcessorError::WrongState),
        }
    }

    pub fn accept(&mut self, now_ms: u64, body: Option<(ContentType, Vec<u8>)>) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::Connecting { transaction } => {
                log::info!("[CallInProcessor] accept call {:?}", body);
                transaction.on_event(now_ms, ServerInviteTransactionEvent::Status(StatusCode::OK, body));
                Ok(())
            }
            _ => Err(super::ProcessorError::WrongState),
        }
    }

    pub fn reject(&mut self, now_ms: u64) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::Connecting { transaction } => {
                log::info!("[CallInProcessor] reject call");
                transaction.on_event(now_ms, ServerInviteTransactionEvent::Status(StatusCode::BusyHere, None));
                Ok(())
            }
            _ => Err(super::ProcessorError::WrongState),
        }
    }

    pub fn end(&mut self, now_ms: u64) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::InCall { .. } => {
                let req = self.create_request(Method::Bye, typed::CSeq { seq: 1, method: Method::Bye }.into());
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

    fn process_transaction(&mut self, now_ms: u64) {
        let transaction = match &mut self.state {
            State::Connecting { transaction } => transaction,
            _ => return,
        };

        while let Some(action) = transaction.pop_action() {
            match action {
                ServerInviteTransactionAction::Res(addr, res) => {
                    self.actions.push_back(ProcessorAction::SendResponse(addr, res));
                }
                ServerInviteTransactionAction::Terminated(Terminated::Accepted(body)) => {
                    self.state = State::InCall {
                        timer_resend_res: Some((now_ms + T1, body)),
                    };
                    break;
                }
                ServerInviteTransactionAction::Terminated(Terminated::Rejected { success: _ }) => {
                    self.state = State::End;
                    self.actions.push_back(ProcessorAction::Finished(Ok(())));
                    break;
                }
            }
        }
    }

    fn create_request(&self, method: Method, cseq: headers::CSeq) -> SipRequest {
         //TODO fix with real hostname
        let request = rsip::Request {
            method,
            uri: rsip::Uri {
                scheme: Some(Scheme::Sip),
                auth: None,
                host_with_port: HostWithPort {
                    host: "proxy.bluesea.live".into(),
                    port: None,
                },
                params: vec![],
                headers: vec![],
            },
            version: rsip::Version::V2,
            headers: Headers::from(vec![
                rsip::Header::Via(headers::Via::from(format!("SIP/2.0/UDP sip-proxy.8xff.live:5060;branch=z9hG4bK-{}", generate_random_string(8)))),
                rsip::Header::MaxForwards(headers::MaxForwards::from(70)),
                rsip::Header::From(headers::From::from(self.init_req.to.to_string())),
                rsip::Header::To(headers::To::from(self.init_req.from.to_string())),
                rsip::Header::CallId(self.init_req.call_id.clone()),
                rsip::Header::CSeq(cseq),
                rsip::Header::ContentLength(headers::ContentLength::from(0)),
            ]),
            body: vec![],
        };
        SipRequest::from(request).expect("Should be valid request")
    }
}

impl Processor<CallInProcessorAction> for CallInProcessor {
    fn start(&mut self, _now_ms: u64) -> Result<(), super::ProcessorError> {
        Ok(())
    }

    fn on_tick(&mut self, now_ms: u64) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::Connecting { transaction } => {
                transaction.on_event(now_ms, ServerInviteTransactionEvent::Timer);
                self.process_transaction(now_ms);
            }
            State::InCall { timer_resend_res } => {
                if let Some((timer_resend_res, body)) = timer_resend_res {
                    if now_ms > *timer_resend_res {
                        let res = ServerInviteTransaction::build_response(&self.init_req, &self.local_contact, StatusCode::OK, body.clone());
                        self.actions.push_back(ProcessorAction::SendResponse(self.remote_contact_addr, res));
                        *timer_resend_res = now_ms + T1;
                    }
                }
            }
            State::Bye { timer_resend_res, timeout } => {
                if now_ms >= *timeout {
                    self.state = State::End;
                    //TODO avoid texting error
                    self.actions.push_back(ProcessorAction::Finished(Err("TIMEOUT".to_string())));
                } else if now_ms >= *timer_resend_res {
                    *timer_resend_res = now_ms + T1;
                    let req = self.create_request(Method::Bye, typed::CSeq { seq: 1, method: Method::Bye }.into());
                    self.actions.push_back(ProcessorAction::SendRequest(self.remote_contact_addr, req));
                }
            }
            State::End => {}
        }
        Ok(())
    }

    fn on_req(&mut self, now_ms: u64, req: crate::sip_request::SipRequest) -> Result<(), super::ProcessorError> {
        if let Some(contact) = req.header_contact() {
            if let Some(addr) = contact.uri.host_with_port.try_into().ok() {
                self.remote_contact_addr = Some(addr);
                log::info!("[CallInProcessor] update sip dest to {}", addr);
            }
        }

        match &mut self.state {
            State::Connecting { transaction } => {
                transaction.on_event(now_ms, ServerInviteTransactionEvent::Req(req));
                self.process_transaction(now_ms);
            }
            State::InCall { timer_resend_res } => match req.method() {
                Method::Ack => {
                    *timer_resend_res = None;
                }
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

    fn on_res(&mut self, _now_ms: u64, res: crate::sip_response::SipResponse) -> Result<(), super::ProcessorError> {
        match &mut self.state {
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

    fn pop_action(&mut self) -> Option<ProcessorAction<CallInProcessorAction>> {
        self.actions.pop_front()
    }
}
