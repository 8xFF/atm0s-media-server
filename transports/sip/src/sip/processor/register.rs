use std::collections::VecDeque;

use rsip::{prelude::ToTypedHeader, typed::Allow, Method, StatusCode};

use crate::sip::{sip_request::SipRequest, sip_response::SipResponse};

use super::{Processor, ProcessorAction};

pub enum RegisterProcessorAction {
    Validate(String),
}

pub struct RegisterProcessor {
    #[allow(unused)]
    started_ms: u64,
    actions: VecDeque<ProcessorAction<RegisterProcessorAction>>,
    init_req: SipRequest,
}

impl RegisterProcessor {
    pub fn new(now_ms: u64, init_req: SipRequest) -> Self {
        Self {
            started_ms: now_ms,
            actions: VecDeque::new(),
            init_req,
        }
    }

    pub fn accept(&mut self, accept: bool) {
        if accept {
            let mut res = self.init_req.build_response(StatusCode::OK, None);
            res.raw
                .headers
                .push(rsip::Header::Allow(Allow(vec![Method::Invite, Method::Ack, Method::Cancel, Method::Options, Method::Bye]).into()));
            self.actions.push_back(ProcessorAction::SendResponse(None, res));
            self.actions.push_back(ProcessorAction::Finished(Ok(())));
        } else {
            let mut res = self.init_req.build_response(StatusCode::Forbidden, None);
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
        if let Some(authorization) = self.init_req.header_authorization() {
            // TODO check authorization
            if let Ok(auth) = authorization.clone().into_typed() {
                self.actions.push_back(ProcessorAction::LogicOutput(RegisterProcessorAction::Validate(auth.username)));
                Ok(())
            } else {
                let mut res = self.init_req.build_response(StatusCode::Unauthorized, None);
                res.raw.headers.push(
                    rsip::typed::WwwAuthenticate {
                        realm: "atlanta.example.com".into(),
                        nonce: "ea9c8e88df84f1cec4341ae6cbe5a359".into(),
                        algorithm: Some(rsip::headers::auth::Algorithm::Md5),
                        qop: Some(rsip::headers::auth::Qop::Auth),
                        stale: Some("FALSE".into()),
                        opaque: Some("".into()),
                        ..Default::default()
                    }
                    .into(),
                );
                self.actions.push_back(ProcessorAction::SendResponse(None, res));
                Ok(())
            }
        } else {
            let mut res = self.init_req.build_response(StatusCode::Unauthorized, None);
            res.raw.headers.push(
                rsip::typed::WwwAuthenticate {
                    realm: "atlanta.example.com".into(),
                    nonce: "ea9c8e88df84f1cec4341ae6cbe5a359".into(),
                    algorithm: Some(rsip::headers::auth::Algorithm::Md5),
                    qop: Some(rsip::headers::auth::Qop::Auth),
                    stale: Some("FALSE".into()),
                    opaque: Some("".into()),
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
        self.init_req = req;
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
