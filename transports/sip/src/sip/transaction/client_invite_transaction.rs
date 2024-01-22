/*


                               |INVITE from TU
             Timer A fires     |INVITE sent
             Reset A,          V                      Timer B fires
             INVITE sent +-----------+                or Transport Err.
               +---------|           |---------------+inform TU
               |         |  Calling  |               |
               +-------->|           |-------------->|
                         +-----------+ 2xx           |
                            |  |       2xx to TU     |
                            |  |1xx                  |
    300-699 +---------------+  |1xx to TU            |
   ACK sent |                  |                     |
resp. to TU |  1xx             V                     |
            |  1xx to TU  -----------+               |
            |  +---------|           |               |
            |  |         |Proceeding |-------------->|
            |  +-------->|           | 2xx           |
            |            +-----------+ 2xx to TU     |
            |       300-699    |                     |
            |       ACK sent,  |                     |
            |       resp. to TU|                     |
            |                  |                     |      NOTE:
            |  300-699         V                     |
            |  ACK sent  +-----------+Transport Err. |  transitions
            |  +---------|           |Inform TU      |  labeled with
            |  |         | Completed |-------------->|  the event
            |  +-------->|           |               |  over the action
            |            +-----------+               |  to take
            |              ^   |                     |
            |              |   | Timer D fires       |
            +--------------+   | -                   |
                               |                     |
                               V                     |
                         +-----------+               |
                         |           |               |
                         | Terminated|<--------------+
                         |           |
                         +-----------+

                 Figure 5: INVITE client transaction
 */

use std::{collections::VecDeque, net::SocketAddr};

use rsip::{
    headers::{CallId, ContentType},
    typed::{Allow, Contact, From, MediaType, To},
    Method, Param,
};

use crate::{sip::utils::generate_random_string, sip_request::SipRequest, sip_response::SipResponse};

const T1: u64 = 500;
const T2: u64 = 64 * T1;

#[derive(Clone)]
struct Calling {
    created_at: u64,
    timer_a_duration: u64,
    timer_a: u64,
    timer_b: u64,
}

#[derive(Clone)]
struct Proceeding {}

#[derive(Clone)]
struct Completed {
    timer_d: u64,
}

#[derive(Clone)]
enum State {
    Calling(Calling),
    Proceeding(Proceeding),
    Completed(Completed),
    Terminated,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Terminated {
    Accepted(Option<(ContentType, Vec<u8>)>),
    Timeout,
    Rejected,
}

pub enum ClientInviteTransactionEvent {
    Timer,
    Res(SipResponse),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClientInviteTransactionAction {
    Req(Option<SocketAddr>, SipRequest),
    Terminated(Terminated),
}

#[allow(unused)]
pub struct ClientInviteTransaction {
    call_id: CallId,
    local_contact: Contact,
    local_from: From,
    remote_to: To,
    pub(crate) origin_request: SipRequest,
    state: State,
    actions: VecDeque<ClientInviteTransactionAction>,
}

impl ClientInviteTransaction {
    pub fn new(now_ms: u64, call_id: CallId, local_contact: Contact, local_from: From, remote_to: To, sdp: &str) -> Self {
        Self {
            origin_request: Self::create_invite_request(call_id.clone(), local_contact.clone(), local_from.clone(), remote_to.clone(), sdp),
            local_contact,
            call_id,
            local_from,
            remote_to,
            state: State::Calling(Calling {
                created_at: now_ms,
                timer_a_duration: T1,
                timer_a: now_ms + T1,
                timer_b: now_ms + T2,
            }),
            actions: VecDeque::new(),
        }
    }

    pub fn on_event(&mut self, now_ms: u64, event: ClientInviteTransactionEvent) {
        match self.state.clone() {
            State::Calling(state) => {
                self.on_calling(state, now_ms, event);
            }
            State::Proceeding(state) => {
                self.on_proceeding(state, now_ms, event);
            }
            State::Completed(state) => {
                self.on_completed(state, now_ms, event);
            }
            State::Terminated => {
                self.on_terminated((), now_ms, event);
            }
        }
    }

    pub fn pop_action(&mut self) -> Option<ClientInviteTransactionAction> {
        self.actions.pop_front()
    }

    /* private */
    fn on_calling(&mut self, state: Calling, now_ms: u64, event: ClientInviteTransactionEvent) {
        match event {
            ClientInviteTransactionEvent::Timer => {
                if now_ms >= state.timer_b {
                    //This is timeout => should switch to Terminated
                    self.state = State::Terminated;
                    self.actions.push_back(ClientInviteTransactionAction::Terminated(Terminated::Timeout));
                } else if now_ms >= state.timer_a {
                    let new_duration = T2.min(state.timer_a_duration * 2);
                    self.state = State::Calling(Calling {
                        created_at: state.created_at,
                        timer_a_duration: new_duration,
                        timer_a: now_ms + new_duration,
                        timer_b: state.timer_b,
                    });
                    self.actions.push_back(ClientInviteTransactionAction::Req(None, self.origin_request.clone()));
                }
            }
            ClientInviteTransactionEvent::Res(res) => match res.raw.status_code().kind() {
                rsip::StatusCodeKind::Provisional => {
                    self.state = State::Proceeding(Proceeding {});
                    let ack = self.create_ack(&res);
                    self.actions.push_back(ClientInviteTransactionAction::Req(None, ack));
                }
                rsip::StatusCodeKind::Successful => {
                    //TODO dont send ack here
                    let ack = self.create_ack(&res);
                    self.actions.push_back(ClientInviteTransactionAction::Req(None, ack));

                    self.state = State::Terminated;
                    let body = res.content_type().map(|ct| (ct.clone(), res.raw.body.clone()));
                    self.actions.push_back(ClientInviteTransactionAction::Terminated(Terminated::Accepted(body)));
                }
                rsip::StatusCodeKind::Redirection | rsip::StatusCodeKind::RequestFailure | rsip::StatusCodeKind::ServerFailure => {
                    self.state = State::Completed(Completed { timer_d: now_ms + T1 });
                    let ack = self.create_ack(&res);
                    self.actions.push_back(ClientInviteTransactionAction::Req(None, ack));
                }
                _ => {}
            },
        }
    }

    fn on_proceeding(&mut self, _state: Proceeding, now_ms: u64, event: ClientInviteTransactionEvent) {
        match event {
            ClientInviteTransactionEvent::Timer => {}
            ClientInviteTransactionEvent::Res(res) => match res.raw.status_code().kind() {
                rsip::StatusCodeKind::Successful => {
                    //TODO dont send ack here
                    let ack = self.create_ack(&res);
                    self.actions.push_back(ClientInviteTransactionAction::Req(None, ack));

                    self.state = State::Terminated;
                    let body = res.content_type().map(|ct| (ct.clone(), res.raw.body.clone()));
                    self.actions.push_back(ClientInviteTransactionAction::Terminated(Terminated::Accepted(body)));
                }
                rsip::StatusCodeKind::Redirection | rsip::StatusCodeKind::RequestFailure | rsip::StatusCodeKind::ServerFailure => {
                    self.state = State::Completed(Completed { timer_d: now_ms + T1 });
                    let ack = self.create_ack(&res);
                    self.actions.push_back(ClientInviteTransactionAction::Req(None, ack));
                }
                _ => {}
            },
        }
    }

    fn on_completed(&mut self, state: Completed, now_ms: u64, event: ClientInviteTransactionEvent) {
        match event {
            ClientInviteTransactionEvent::Timer => {
                if now_ms >= state.timer_d {
                    self.state = State::Terminated;
                    self.actions.push_back(ClientInviteTransactionAction::Terminated(Terminated::Rejected));
                }
            }
            ClientInviteTransactionEvent::Res(res) => match res.raw.status_code().kind() {
                rsip::StatusCodeKind::Redirection => {
                    let ack = self.create_ack(&res);
                    self.actions.push_back(ClientInviteTransactionAction::Req(None, ack));
                }
                _ => {}
            },
        }
    }

    fn on_terminated(&mut self, _state: (), _now_ms: u64, _event: ClientInviteTransactionEvent) {}

    fn create_invite_request(call_id: CallId, local_contact: Contact, local_from: From, remote_to: To, local_sdp: &str) -> SipRequest {
        let body = local_sdp.as_bytes().to_vec();

        //TODO fix with real hostname
        let request = rsip::Request {
            method: rsip::Method::Invite,
            uri: rsip::Uri {
                scheme: Some(rsip::Scheme::Sip),
                auth: remote_to.uri.auth.clone(),
                host_with_port: remote_to.uri.host_with_port.clone(),
                params: vec![Param::Transport(rsip::Transport::Udp)],
                headers: vec![],
            },
            version: rsip::Version::V2,
            headers: rsip::Headers::from(vec![
                rsip::Header::Via(rsip::headers::Via::from(format!(
                    "SIP/2.0/UDP {}:{};branch=z9hG4bK-{}",
                    local_contact.uri.host(),
                    local_contact.uri.port().unwrap_or(&(5060.into())),
                    generate_random_string(8)
                ))),
                rsip::Header::MaxForwards(rsip::headers::MaxForwards::from(70)),
                rsip::Header::Contact(local_contact.into()),
                rsip::Header::From(local_from.into()),
                rsip::Header::To(remote_to.into()),
                rsip::Header::CallId(call_id),
                rsip::Header::CSeq(rsip::typed::CSeq { seq: 1, method: rsip::Method::Invite }.into()),
                rsip::Header::Allow(Allow(vec![Method::Invite, Method::Ack, Method::Cancel, Method::Options, Method::Bye]).into()),
                rsip::Header::ContentLength(rsip::headers::ContentLength::from(body.len() as u32)),
                rsip::Header::ContentType(rsip::typed::ContentType(MediaType::Sdp(vec![])).into()),
                rsip::Header::UserAgent(rsip::headers::UserAgent::from("8xff-sip-media-server")),
            ]),
            body,
        };
        SipRequest::from(request).expect("Should be valid request")
    }

    fn create_ack(&mut self, for_res: &SipResponse) -> SipRequest {
        //TODO fix with real hostname
        let request = rsip::Request {
            method: rsip::Method::Ack,
            uri: self.origin_request.raw.uri.clone(),
            version: rsip::Version::V2,
            headers: rsip::Headers::from(vec![
                rsip::Header::Via(self.origin_request.via.clone().into()),
                rsip::Header::MaxForwards(rsip::headers::MaxForwards::from(70)),
                rsip::Header::From(self.origin_request.from.clone().into()),
                rsip::Header::To(for_res.to.clone().into()),
                rsip::Header::CallId(self.origin_request.call_id.clone().into()),
                rsip::Header::CSeq(rsip::typed::CSeq { seq: 1, method: rsip::Method::Ack }.into()),
                rsip::Header::ContentLength(rsip::headers::ContentLength::from(0)),
                rsip::Header::UserAgent(rsip::headers::UserAgent::from("8xff-sip-media-server")),
            ]),
            body: vec![],
        };
        SipRequest::from(request).expect("Should be valid request")
    }
}

//TODO test same with server_invite
