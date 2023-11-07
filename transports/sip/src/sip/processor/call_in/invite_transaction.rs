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

pub enum ServerInviteTransactionAction {
    Res(Option<SocketAddr>, SipResponse),
    Terminated(Option<(ContentType, Vec<u8>)>),
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
                    self.actions.push_back(ServerInviteTransactionAction::Terminated(body.clone()));
                    self.response(status, body);
                } else {
                }
            }
        }
    }

    fn on_completed(&mut self, state: Completed, now_ms: u64, event: ServerInviteTransactionEvent) {
        match event {
            ServerInviteTransactionEvent::Timer => {
                if now_ms > state.timer_g {
                    log::info!("[ServerInviteTransacstion:on_completed] dont received ACK after timeout => resend response {}", state.code);
                    self.response(state.code.clone(), None);
                    let timer_g_duration = T2.min(2 * state.timer_g_duration);
                    self.state = State::Completed(Completed {
                        code: state.code,
                        timer_g: now_ms + timer_g_duration,
                        timer_g_duration,
                        timer_h: state.timer_h,
                    });
                } else if now_ms > state.timer_h {
                    log::info!("[ServerInviteTransacstion:on_completed] dont received ACK after long timeout => switched to Terminated");
                    self.state = State::Terminated;
                    self.actions.push_back(ServerInviteTransactionAction::Terminated(None));
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
                if now_ms > state.timer_i {
                    log::info!("[ServerInviteTransacstion:on_confirmed] after timeout_i => switched to Terminated");
                    self.state = State::Terminated;
                    self.actions.push_back(ServerInviteTransactionAction::Terminated(None));
                }
            }
            ServerInviteTransactionEvent::Req(_) => {}
            ServerInviteTransactionEvent::Status(_, _) => {}
        }
    }

    fn on_terminated(&mut self, state: (), now_ms: u64, event: ServerInviteTransactionEvent) {}
}
