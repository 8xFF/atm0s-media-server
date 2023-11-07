/*

                              |INVITE
                              |pass INV to TU
           INVITE             V send 100 if TU won't in 200ms
           send response+-----------+
               +--------|           |--------+101-199 from TU
               |        | Proceeding|        |send response
               +------->|           |<-------+
                        |           |          Transport Err.
                        |           |          Inform TU
                        |           |--------------->+
                        +-----------+                |
           300-699 from TU |     |2xx from TU        |
           send response   |     |send response      |
                           |     +------------------>+
                           |                         |
           INVITE          V          Timer G fires  |
           send response+-----------+ send response  |
               +--------|           |--------+       |
               |        | Completed |        |       |
               +------->|           |<-------+       |
                        +-----------+                |
                           |     |                   |
                       ACK |     |                   |
                       -   |     +------------------>+
                           |        Timer H fires    |
                           V        or Transport Err.|
                        +-----------+  Inform TU     |
                        |           |                |
                        | Confirmed |                |
                        |           |                |
                        +-----------+                |
                              |                      |
                              |Timer I fires         |
                              |-                     |
                              |                      |
                              V                      |
                        +-----------+                |
                        |           |                |
                        | Terminated|<---------------+
                        |           |
                        +-----------+
*/

use std::{collections::VecDeque, net::SocketAddr};

use rsip::{
    headers::ContentType,
    typed::{Allow, Contact},
    Method, StatusCode, StatusCodeKind,
};

use crate::{sip_request::SipRequest, sip_response::SipResponse};

const TU_100_AFTER_MS: u64 = 200;
const T1: u64 = 500;
const T2: u64 = 500 * 64;

#[derive(Clone)]
struct Proceeding {
    tu_100_timer: Option<u64>,
}
#[derive(Clone)]
struct Completed {
    code: StatusCode,
    timer_g: u64,
    timer_g_duration: u64,
    timer_h: u64,
}
#[derive(Clone)]
struct Confirmed {
    timer_i: u64,
}

#[derive(Clone)]
enum State {
    Proceeding(Proceeding),
    Completed(Completed),
    Confirmed(Confirmed),
    Terminated,
}

pub enum ServerInviteTransactionEvent {
    Timer,
    Req(SipRequest),
    Status(StatusCode, Option<(ContentType, Vec<u8>)>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Terminated {
    Accepted(Option<(ContentType, Vec<u8>)>),
    Rejected { success: bool },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ServerInviteTransactionAction {
    Res(Option<SocketAddr>, SipResponse),
    Terminated(Terminated),
}

pub struct ServerInviteTransaction {
    init_req: SipRequest,
    state: State,
    local_contact: Contact,
    actions: VecDeque<ServerInviteTransactionAction>,
}

impl ServerInviteTransaction {
    pub fn new(now_ms: u64, local_contact: Contact, init_req: SipRequest) -> Self {
        Self {
            local_contact,
            init_req,
            state: State::Proceeding(Proceeding {
                tu_100_timer: Some(now_ms + TU_100_AFTER_MS),
            }),
            actions: VecDeque::new(),
        }
    }

    pub fn build_response(init_req: &SipRequest, local_contact: &Contact, code: StatusCode, body: Option<(ContentType, Vec<u8>)>) -> SipResponse {
        let mut res = init_req.build_response(code.clone(), body);
        for header in init_req.raw.headers.iter() {
            match header {
                rsip::Header::RecordRoute(header) => {
                    res.raw.headers.push(rsip::Header::RecordRoute(header.clone()));
                }
                _ => {}
            }
        }

        if let 101..=399 | 485 = code.code() {
            res.raw.headers.push(rsip::Header::Contact(local_contact.clone().into()));
        }

        if let 180..=189 | 200..=299 | 405 = code.code() {
            res.raw
                .headers
                .push(rsip::Header::Allow(Allow(vec![Method::Invite, Method::Ack, Method::Cancel, Method::Options, Method::Bye]).into()));
        }

        // if let 200..=299 = code.code() {
        //     if init_req.to.tag().is_none() {
        //         // Add To-tag to success response to create dialog
        //         for header in res.raw.headers.iter_mut() {
        //             match header {
        //                 rsip::Header::To(header) => {
        //                     header.mut_tag(tag);
        //                 }
        //                 _ => {}
        //             }
        //         }
        //     }
        // }
        res
    }

    fn response(&mut self, code: StatusCode, body: Option<(ContentType, Vec<u8>)>) {
        let res = Self::build_response(&self.init_req, &self.local_contact, code, body);
        self.actions.push_back(ServerInviteTransactionAction::Res(None, res));
    }

    pub fn on_event(&mut self, now_ms: u64, event: ServerInviteTransactionEvent) {
        match self.state.clone() {
            State::Proceeding(state) => {
                self.on_proceeding(state, now_ms, event);
            }
            State::Completed(state) => {
                self.on_completed(state, now_ms, event);
            }
            State::Confirmed(state) => {
                self.on_confirmed(state, now_ms, event);
            }
            State::Terminated => {
                self.on_terminated((), now_ms, event);
            }
        }
    }

    pub fn pop_action(&mut self) -> Option<ServerInviteTransactionAction> {
        self.actions.pop_front()
    }

    /* Now is private state processor */

    fn on_proceeding(&mut self, mut state: Proceeding, now_ms: u64, event: ServerInviteTransactionEvent) {
        match event {
            ServerInviteTransactionEvent::Timer => {
                if let Some(timeout) = state.tu_100_timer {
                    if now_ms >= timeout {
                        log::info!("[ServerInviteTransacstion:on_proceeding] send 100 Trying");
                        self.response(StatusCode::Trying, None);
                        state.tu_100_timer = None;
                        self.state = State::Proceeding(state);
                    }
                }
            }
            ServerInviteTransactionEvent::Req(req) => {
                if req.method().eq(&Method::Cancel) {
                    log::info!("[ServerInviteTransacstion:on_proceeding] received Cancel => send 200, RequestTerminated and switch to Completed");
                    self.state = State::Completed(Completed {
                        code: StatusCode::RequestTerminated,
                        timer_g: now_ms + T1,
                        timer_g_duration: T1,
                        timer_h: now_ms + T2,
                    });
                    let cancel_res = req.build_response(StatusCode::OK, None);
                    self.actions.push_back(ServerInviteTransactionAction::Res(None, cancel_res));
                    self.response(StatusCode::RequestTerminated, None);
                }
            }
            ServerInviteTransactionEvent::Status(status, body) => {
                if matches!(status.kind(), StatusCodeKind::Provisional) {
                    state.tu_100_timer = None;
                    self.state = State::Proceeding(state);
                    self.response(status, body);
                } else if let 300..=699 = status.code() {
                    self.state = State::Completed(Completed {
                        code: status.clone(),
                        timer_g: now_ms + T1,
                        timer_g_duration: T1,
                        timer_h: now_ms + T2,
                    });
                    self.response(status, body);
                } else if matches!(status.kind(), StatusCodeKind::Successful) {
                    state.tu_100_timer = None;
                    self.state = State::Terminated;
                    self.response(status, body.clone());
                    self.actions.push_back(ServerInviteTransactionAction::Terminated(Terminated::Accepted(body)));
                } else {
                }
            }
        }
    }

    fn on_completed(&mut self, state: Completed, now_ms: u64, event: ServerInviteTransactionEvent) {
        match event {
            ServerInviteTransactionEvent::Timer => {
                if now_ms >= state.timer_h {
                    log::info!("[ServerInviteTransacstion:on_completed] dont received ACK after long timeout => switched to Terminated");
                    self.state = State::Terminated;
                    self.actions.push_back(ServerInviteTransactionAction::Terminated(Terminated::Rejected { success: false }));
                } else if now_ms >= state.timer_g {
                    log::info!("[ServerInviteTransacstion:on_completed] dont received ACK after timeout => resend response {}", state.code);
                    self.response(state.code.clone(), None);
                    let timer_g_duration = T2.min(2 * state.timer_g_duration);
                    self.state = State::Completed(Completed {
                        code: state.code,
                        timer_g: now_ms + timer_g_duration,
                        timer_g_duration,
                        timer_h: state.timer_h,
                    });
                }
            }
            ServerInviteTransactionEvent::Req(req) => {
                if req.method().eq(&Method::Ack) {
                    log::info!("[ServerInviteTransacstion:on_completed] received ACK => switched to Confirmed");
                    self.state = State::Confirmed(Confirmed { timer_i: now_ms + T1 });
                }
            }
            ServerInviteTransactionEvent::Status(_, _) => {}
        }
    }

    fn on_confirmed(&mut self, state: Confirmed, now_ms: u64, event: ServerInviteTransactionEvent) {
        match event {
            ServerInviteTransactionEvent::Timer => {
                if now_ms >= state.timer_i {
                    log::info!("[ServerInviteTransacstion:on_confirmed] after timeout_i => switched to Terminated");
                    self.state = State::Terminated;
                    self.actions.push_back(ServerInviteTransactionAction::Terminated(Terminated::Rejected { success: true }));
                }
            }
            ServerInviteTransactionEvent::Req(_) => {}
            ServerInviteTransactionEvent::Status(_, _) => {}
        }
    }

    fn on_terminated(&mut self, _state: (), _now_ms: u64, _event: ServerInviteTransactionEvent) {}
}

#[cfg(test)]
mod test {
    use rsip::{headers::Contact, prelude::HeadersExt, StatusCode};

    use crate::{
        sip::transaction::server_invite_transaction::{ServerInviteTransactionAction, ServerInviteTransactionEvent, Terminated},
        sip_request::SipRequest,
    };

    use super::{ServerInviteTransaction, T1};

    macro_rules! cast2 {
        ($target: expr, $pat: path) => {{
            let v = $target;
            match v {
                $pat(a, b) => (a, b),
                _ => panic!("mismatch variant when cast to {} got {:?}", stringify!($pat), v),
            }
        }};
    }

    const INVITE_REQ: &str = "INVITE sip:1003@192.168.66.113;transport=UDP SIP/2.0\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---4900d58f2225595c;rport\r
Max-Forwards: 70\r
Contact: <sip:1002@192.168.66.155:59530;transport=UDP>\r
To: <sip:1003@192.168.66.113>\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=b3b27614\r
Call-ID: bDioe0g_lGydVf71NpTBnA..\r
CSeq: 1 INVITE\r
Allow: INVITE, ACK, CANCEL, BYE, NOTIFY, REFER, MESSAGE, OPTIONS, INFO, SUBSCRIBE\r
Content-Type: application/sdp\r
Supported: replaces, norefersub, extended-refer, timer, sec-agree, outbound, path, X-cisco-serviceuri\r
User-Agent: Zoiper v2.10.19.5\r
Allow-Events: presence, kpml, talk, as-feature-event\r
Content-Length: 264\r
\r
v=0\r
o=Z 0 199267607 IN IP4 192.168.66.155\r
s=Z\r
c=IN IP4 192.168.66.155\r
t=0 0\r
m=audio 61265 RTP/AVP 3 101 110 97 8 0\r
a=rtpmap:101 telephone-event/8000\r
a=fmtp:101 0-16\r
a=rtpmap:110 speex/8000\r
a=rtpmap:97 iLBC/8000\r
a=fmtp:97 mode=20\r
a=sendrecv\r
a=rtcp-mux\r
";

    const ACK_REQ: &str = "ACK sip:192.168.66.113 SIP/2.0\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---3c9aaece04169f91;rport\r
Max-Forwards: 70\r
Contact: <sip:1002@192.168.66.155:59530;transport=UDP>\r
To: <sip:1003@192.168.66.113>\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=b3b27614\r
Call-ID: bDioe0g_lGydVf71NpTBnA..\r
CSeq: 1 ACK\r
User-Agent: Zoiper v2.10.19.5\r
Content-Length: 0\r\n\r\n";

    const CANCEL_REQ: &str = "CANCEL sip:192.168.66.113 SIP/2.0\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---3c9aaece04169f91;rport\r
Max-Forwards: 70\r
Contact: <sip:1002@192.168.66.155:59530;transport=UDP>\r
To: <sip:1003@192.168.66.113>\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=b3b27614\r
Call-ID: bDioe0g_lGydVf71NpTBnA..\r
CSeq: 1 CANCEL\r
User-Agent: Zoiper v2.10.19.5\r
Content-Length: 0\r\n\r\n";

    #[test]
    fn simple_success() {
        let local_contact = Contact::try_from("sip:127.0.0.1:5060").expect("Should ok");
        let init_req = SipRequest::from(rsip::Request::try_from(INVITE_REQ).expect("Should work")).expect("Should parse");
        let mut transaction = ServerInviteTransaction::new(0, local_contact.try_into().expect("Should ok"), init_req);

        assert_eq!(transaction.pop_action(), None);

        transaction.on_event(T1, ServerInviteTransactionEvent::Timer);
        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::Trying);

        assert_eq!(transaction.pop_action(), None);

        transaction.on_event(T1 + 200, ServerInviteTransactionEvent::Status(StatusCode::Ringing, None));
        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::Ringing);
        assert_eq!(transaction.pop_action(), None);

        transaction.on_event(T1 + 400, ServerInviteTransactionEvent::Status(StatusCode::OK, None));
        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::OK);
        assert_eq!(transaction.pop_action(), Some(ServerInviteTransactionAction::Terminated(Terminated::Accepted(None))));
        assert_eq!(transaction.pop_action(), None);
    }

    #[test]
    fn simple_reject() {
        let local_contact = Contact::try_from("sip:127.0.0.1:5060").expect("Should ok");
        let init_req = SipRequest::from(rsip::Request::try_from(INVITE_REQ).expect("Should work")).expect("Should parse");
        let mut transaction = ServerInviteTransaction::new(0, local_contact.try_into().expect("Should ok"), init_req);

        assert_eq!(transaction.pop_action(), None);
        transaction.on_event(20, ServerInviteTransactionEvent::Status(StatusCode::BusyHere, None));
        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::BusyHere);

        // resend if not received ack
        transaction.on_event(T1 + 20, ServerInviteTransactionEvent::Timer);
        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::BusyHere);
        assert_eq!(transaction.pop_action(), None);

        // resend in next 2 * T1 if not received ack
        transaction.on_event(3 * T1 + 20, ServerInviteTransactionEvent::Timer);
        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::BusyHere);
        assert_eq!(transaction.pop_action(), None);

        // received ack => end
        let ack_req = SipRequest::from(rsip::Request::try_from(ACK_REQ).expect("Should parse")).expect("Should parse");
        transaction.on_event(3 * T1 + 30, ServerInviteTransactionEvent::Req(ack_req));
        assert_eq!(transaction.pop_action(), None);

        // after timer_i => terminated
        transaction.on_event(3 * T1 + 30 + T1, ServerInviteTransactionEvent::Timer);

        assert_eq!(transaction.pop_action(), Some(ServerInviteTransactionAction::Terminated(Terminated::Rejected { success: true })));
        assert_eq!(transaction.pop_action(), None);
    }

    #[test]
    fn simple_cancel() {
        let local_contact = Contact::try_from("sip:127.0.0.1:5060").expect("Should ok");
        let init_req = SipRequest::from(rsip::Request::try_from(INVITE_REQ).expect("Should work")).expect("Should parse");
        let mut transaction = ServerInviteTransaction::new(0, local_contact.try_into().expect("Should ok"), init_req);

        assert_eq!(transaction.pop_action(), None);

        let cancel_req = SipRequest::from(rsip::Request::try_from(CANCEL_REQ).expect("Should parse")).expect("Should parse");
        transaction.on_event(20, ServerInviteTransactionEvent::Req(cancel_req));

        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::OK);
        assert_eq!(res.raw.cseq_header(), Ok(&rsip::headers::CSeq::from("1 CANCEL")));

        let (dest, res) = cast2!(transaction.pop_action().expect("Should have action"), ServerInviteTransactionAction::Res);
        assert_eq!(dest, None);
        assert_eq!(res.raw.status_code, StatusCode::RequestTerminated);
        assert_eq!(res.raw.cseq_header(), Ok(&rsip::headers::CSeq::from("1 INVITE")));

        assert_eq!(transaction.pop_action(), None);
    }
}
