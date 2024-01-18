use std::{collections::VecDeque, net::SocketAddr};

use rsip::{
    headers::{self, typed, ContentType},
    typed::Contact,
    Headers, Method, Param, Scheme, StatusCode,
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

#[derive(Debug, PartialEq, Eq)]
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
                self.process_transaction(now_ms);
                Ok(())
            }
            _ => Err(super::ProcessorError::WrongState),
        }
    }

    pub fn accept(&mut self, now_ms: u64, body: Option<(ContentType, Vec<u8>)>) -> Result<(), super::ProcessorError> {
        match &mut self.state {
            State::Connecting { transaction } => {
                log::info!("[CallInProcessor] accept call");
                transaction.on_event(now_ms, ServerInviteTransactionEvent::Status(StatusCode::OK, body));
                self.process_transaction(now_ms);
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
                self.process_transaction(now_ms);
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

    /// Auto determine the state and send REJECT or BYE
    pub fn close(&mut self, now_ms: u64) {
        match &self.state {
            State::Connecting { .. } => {
                self.reject(now_ms).expect("Should ok");
            }
            State::InCall { .. } => {
                self.end(now_ms).expect("Should ok");
            }
            State::Bye { .. } => {}
            State::End => {}
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
        let request = rsip::Request {
            method,
            uri: rsip::Uri {
                scheme: Some(rsip::Scheme::Sip),
                auth: self.local_contact.uri.auth.clone(),
                host_with_port: self.local_contact.uri.host_with_port.clone(),
                params: vec![Param::Transport(rsip::Transport::Udp)],
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
                rsip::Header::From(headers::From::from(self.init_req.to.to_string())),
                rsip::Header::To(headers::To::from(self.init_req.from.to_string())),
                rsip::Header::CallId(self.init_req.call_id.clone()),
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
                    log::warn!("[CallInProcessor] resend BYE");
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
                match req.method() {
                    Method::Cancel => {
                        let res = req.build_response(StatusCode::OK, None);
                        self.actions.push_back(ProcessorAction::SendResponse(self.remote_contact_addr, res));
                        transaction.on_event(now_ms, ServerInviteTransactionEvent::Status(StatusCode::RequestTerminated, None));
                    }
                    _ => {
                        transaction.on_event(now_ms, ServerInviteTransactionEvent::Req(req));
                    }
                }
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

#[cfg(test)]
mod test {
    use rsip::headers::Contact;

    use crate::{
        processor::{Processor, ProcessorAction},
        sip::data::sip_pkt::{ACK_REQ, BYE_RES, INVITE_REQ},
        sip_request::SipRequest,
        sip_response::SipResponse,
    };

    use super::{CallInProcessor, T1};

    macro_rules! cast2 {
        ($target: expr, $pat: path) => {{
            let v = $target;
            match v {
                $pat(a, b) => (a, b),
                _ => panic!("mismatch variant when cast to {} got {:?}", stringify!($pat), v),
            }
        }};
    }

    #[test]
    fn normal_call() {
        let local_contact = Contact::try_from("sip:127.0.0.1:5060").expect("Should ok");
        let init_req = SipRequest::from(rsip::Request::try_from(INVITE_REQ).expect("Should work")).expect("Should parse");
        let mut processor = CallInProcessor::new(0, local_contact.try_into().expect("Should ok"), init_req);

        processor.start(0).expect("Should ok");

        assert_eq!(processor.pop_action(), None);

        //after timeout T1 without any action should send 100 Trying
        processor.on_tick(T1).expect("Should ok");

        let (_, res) = cast2!(processor.pop_action().expect("Should have action"), ProcessorAction::SendResponse);
        assert_eq!(res.raw.status_code, rsip::StatusCode::Trying);
        assert_eq!(processor.pop_action(), None);

        //after call ringing should send Ringing
        processor.ringing(T1 + 1000).expect("Should ok");
        let (_, res) = cast2!(processor.pop_action().expect("Should have action"), ProcessorAction::SendResponse);
        assert_eq!(res.raw.status_code, rsip::StatusCode::Ringing);
        assert_eq!(processor.pop_action(), None);

        //after call accept should send 200 OK
        processor.accept(T1 + 2000, None).expect("Should ok");
        let (_, res) = cast2!(processor.pop_action().expect("Should have action"), ProcessorAction::SendResponse);
        assert_eq!(res.raw.status_code, rsip::StatusCode::OK);
        assert_eq!(processor.pop_action(), None);

        //after call end should send BYE
        processor.end(T1 + 3000).expect("Should ok");
        let (_, req) = cast2!(processor.pop_action().expect("Should have action"), ProcessorAction::SendRequest);
        assert_eq!(req.method(), &rsip::Method::Bye);
        assert_eq!(processor.pop_action(), None);

        //after received bye response 200 should finish the call
        processor
            .on_res(T1 + 4000, SipResponse::from(rsip::Response::try_from(BYE_RES).expect("Should ok")).expect("Should parse"))
            .expect("Should ok");
        assert_eq!(processor.pop_action(), Some(ProcessorAction::Finished(Ok(()))));
        assert_eq!(processor.pop_action(), None);
    }

    #[test]
    fn reject_call() {
        let local_contact = Contact::try_from("sip:127.0.0.1:5060").expect("Should ok");
        let init_req = SipRequest::from(rsip::Request::try_from(INVITE_REQ).expect("Should work")).expect("Should parse");
        let mut processor = CallInProcessor::new(0, local_contact.try_into().expect("Should ok"), init_req);

        processor.start(0).expect("Should ok");

        assert_eq!(processor.pop_action(), None);

        //after timeout T1 without any action should send 100 Trying
        processor.on_tick(T1).expect("Should ok");

        let (_, res) = cast2!(processor.pop_action().expect("Should have action"), ProcessorAction::SendResponse);
        assert_eq!(res.raw.status_code, rsip::StatusCode::Trying);
        assert_eq!(processor.pop_action(), None);

        //after call ringing should send Ringing
        processor.ringing(T1 + 1000).expect("Should ok");
        let (_, res) = cast2!(processor.pop_action().expect("Should have action"), ProcessorAction::SendResponse);
        assert_eq!(res.raw.status_code, rsip::StatusCode::Ringing);
        assert_eq!(processor.pop_action(), None);

        //after call reject should send 486 BusyHere
        processor.reject(T1 + 2000).expect("Should ok");
        let (_, res) = cast2!(processor.pop_action().expect("Should have action"), ProcessorAction::SendResponse);
        assert_eq!(res.raw.status_code, rsip::StatusCode::BusyHere);

        let ack_req = SipRequest::from(rsip::Request::try_from(ACK_REQ).expect("Should parse")).expect("Should parse");
        processor.on_req(T1 + 3000, ack_req).expect("Should ok");

        //after timeout T1 Should finish
        processor.on_tick(T1 + 3000 + T1).expect("Should ok");

        assert_eq!(processor.pop_action(), Some(ProcessorAction::Finished(Ok(()))));
        assert_eq!(processor.pop_action(), None);
    }

    //TODO test CANCEL_REQ
}
