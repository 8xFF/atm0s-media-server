use std::collections::VecDeque;

use rsip::{prelude::ToTypedHeader, typed::Allow, Method, StatusCode};

use crate::sip::{sip_request::SipRequest, sip_response::SipResponse};

use super::{Processor, ProcessorAction};

pub const REALM: &str = "sip.media-server.8xff.io";

/// random 16 hex string
fn random_nonce() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut nonce = String::new();
    for _ in 0..16 {
        nonce.push_str(&format!("{:x}", rng.gen::<u8>()));
    }
    nonce
}

pub enum RegisterProcessorAction {
    Validate(String, String, String, String, String),
}

pub struct RegisterProcessor {
    #[allow(unused)]
    started_ms: u64,
    actions: VecDeque<ProcessorAction<RegisterProcessorAction>>,
    req: SipRequest,
}

impl RegisterProcessor {
    pub fn new(now_ms: u64, init_req: SipRequest) -> Self {
        Self {
            started_ms: now_ms,
            actions: VecDeque::new(),
            req: init_req,
        }
    }

    pub fn accept(&mut self, accept: bool) {
        if accept {
            let mut res = self.req.build_response(StatusCode::OK, None);
            res.raw
                .headers
                .push(rsip::Header::Allow(Allow(vec![Method::Invite, Method::Ack, Method::Cancel, Method::Options, Method::Bye]).into()));
            self.actions.push_back(ProcessorAction::SendResponse(None, res));
            self.actions.push_back(ProcessorAction::Finished(Ok(())));
        } else {
            let mut res = self.req.build_response(StatusCode::Forbidden, None);
            res.raw
                .headers
                .push(rsip::Header::Allow(Allow(vec![Method::Invite, Method::Ack, Method::Cancel, Method::Options, Method::Bye]).into()));
            self.actions.push_back(ProcessorAction::SendResponse(None, res));
            self.actions.push_back(ProcessorAction::Finished(Err("WrongUser".into())));
        }
    }
}

impl Processor<RegisterProcessorAction> for RegisterProcessor {
    fn start(&mut self, _now_ms: u64) -> Result<(), super::ProcessorError> {
        if let Some(authorization) = self.req.header_authorization() {
            if let Ok(auth) = authorization.clone().into_typed() {
                self.actions.push_back(ProcessorAction::LogicOutput(RegisterProcessorAction::Validate(
                    auth.uri.to_string(),
                    auth.nonce,
                    auth.username,
                    REALM.to_string(),
                    auth.response,
                )));
                Ok(())
            } else {
                let mut res = self.req.build_response(StatusCode::Unauthorized, None);
                res.raw.headers.push(
                    rsip::typed::WwwAuthenticate {
                        realm: REALM.into(),
                        nonce: random_nonce().into(),
                        algorithm: Some(rsip::headers::auth::Algorithm::Md5),
                        qop: None,
                        stale: None,
                        opaque: None,
                        ..Default::default()
                    }
                    .into(),
                );
                self.actions.push_back(ProcessorAction::SendResponse(None, res));
                Ok(())
            }
        } else {
            let mut res = self.req.build_response(StatusCode::Unauthorized, None);
            res.raw.headers.push(
                rsip::typed::WwwAuthenticate {
                    realm: REALM.into(),
                    nonce: random_nonce().into(),
                    algorithm: Some(rsip::headers::auth::Algorithm::Md5),
                    qop: None,
                    stale: None,
                    opaque: None,
                    ..Default::default()
                }
                .into(),
            );
            self.actions.push_back(ProcessorAction::SendResponse(None, res));
            Ok(())
        }
    }

    fn on_tick(&mut self, _now_ms: u64) -> Result<(), super::ProcessorError> {
        //TODO check timeout
        Ok(())
    }

    fn on_req(&mut self, now_ms: u64, req: SipRequest) -> Result<(), super::ProcessorError> {
        self.req = req;
        self.start(now_ms)?;
        Ok(())
    }

    fn on_res(&mut self, _now_ms: u64, _res: SipResponse) -> Result<(), super::ProcessorError> {
        // not used
        Err(super::ProcessorError::WrongMessage)
    }

    fn pop_action(&mut self) -> Option<super::ProcessorAction<RegisterProcessorAction>> {
        self.actions.pop_front()
    }
}

#[cfg(test)]
mod test {}
