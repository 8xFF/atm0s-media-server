use std::fmt::Debug;

use rsip::{
    headers::{Authorization, ContentLength, ContentType, UserAgent},
    prelude::ToTypedHeader,
    typed::Contact,
    StatusCode,
};

use super::sip_response::SipResponse;

#[derive(Debug)]
pub enum RequiredHeader {
    CallId,
    CSeq,
    From,
    To,
    Via,
}

#[derive(Debug)]
pub enum SipRequestError {
    Missing(RequiredHeader),
}

#[derive(Clone, PartialEq, Eq)]
pub struct SipRequest {
    pub raw: rsip::Request,
    pub call_id: rsip::headers::CallId,
    pub cseq: rsip::headers::typed::CSeq,
    pub from: rsip::headers::typed::From,
    pub to: rsip::headers::typed::To,
    pub via: rsip::headers::typed::Via,
    pub timestamp: Option<rsip::headers::Timestamp>,
}

impl Debug for SipRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SipRequest")
            .field("method", &self.raw.method)
            .field("call_id", &self.call_id)
            .field("cseq", &self.cseq)
            .field("from", &self.from)
            .field("to", &self.to)
            .field("via", &self.via)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

impl SipRequest {
    pub fn from(sip_request: rsip::Request) -> Result<Self, SipRequestError> {
        let mut call_id: Option<rsip::headers::CallId> = None;
        let mut cseq: Option<rsip::headers::typed::CSeq> = None;
        let mut from: Option<rsip::headers::typed::From> = None;
        let mut to: Option<rsip::headers::typed::To> = None;
        let mut via: Option<rsip::headers::typed::Via> = None;
        let mut timestamp: Option<rsip::headers::Timestamp> = None;

        for header in sip_request.headers.iter() {
            match header {
                rsip::Header::CallId(header) => {
                    call_id = Some(header.clone());
                }
                rsip::Header::CSeq(header) => {
                    cseq = Some(header.clone().into_typed().expect("Should be valid CSeq"));
                }
                rsip::Header::From(header) => {
                    from = Some(header.clone().into_typed().expect("Should be valid From"));
                }
                rsip::Header::To(header) => {
                    to = Some(header.clone().into_typed().expect("Should be valid To"));
                }
                rsip::Header::Via(header) => {
                    via = Some(header.clone().into_typed().expect("Should be valid Via"));
                }
                rsip::Header::Timestamp(header) => {
                    timestamp = Some(header.clone());
                }
                _ => {}
            }
        }

        if call_id.is_none() {
            return Err(SipRequestError::Missing(RequiredHeader::CallId));
        }
        if cseq.is_none() {
            return Err(SipRequestError::Missing(RequiredHeader::CSeq));
        }
        if from.is_none() {
            return Err(SipRequestError::Missing(RequiredHeader::From));
        }
        if to.is_none() {
            return Err(SipRequestError::Missing(RequiredHeader::To));
        }
        if via.is_none() {
            return Err(SipRequestError::Missing(RequiredHeader::Via));
        }

        Ok(Self {
            raw: sip_request,
            call_id: call_id.expect("Must some"),
            cseq: cseq.expect("Must some"),
            from: from.expect("Must some"),
            to: to.expect("Must some"),
            via: via.expect("Must some"),
            timestamp,
        })
    }

    pub fn method(&self) -> &rsip::Method {
        &self.raw.method
    }

    pub fn digest_uri(&self) -> String {
        self.raw.uri().to_string()
    }

    pub fn body_str(&self) -> String {
        String::from_utf8_lossy(&self.raw.body).to_string()
    }

    pub fn header_authorization(&self) -> Option<&Authorization> {
        for header in self.raw.headers.iter() {
            match header {
                rsip::Header::Authorization(auth) => return Some(auth),
                _ => {}
            }
        }
        None
    }

    pub fn header_contact(&self) -> Option<Contact> {
        for header in self.raw.headers.iter() {
            match header {
                rsip::Header::Contact(contact) => return Some(contact.typed().ok()?),
                _ => {}
            }
        }
        None
    }

    pub fn build_response(&self, code: StatusCode, body: Option<(ContentType, Vec<u8>)>) -> SipResponse {
        let mut headers: rsip::Headers = Default::default();
        headers.push(rsip::Header::Via(self.via.clone().into()));
        headers.push(rsip::Header::From(self.from.clone().into()));
        headers.push(rsip::Header::To(self.to.clone().into()));
        headers.push(rsip::Header::CallId(self.call_id.clone()));
        headers.push(rsip::Header::CSeq(self.cseq.clone().into()));
        let body_len = body.as_ref().map(|(_, body)| body.len()).unwrap_or(0);
        headers.push(rsip::Header::ContentLength(ContentLength::from(body_len as u32)));
        headers.push(rsip::Header::UserAgent(UserAgent::default()));

        if let Some((content_type, _)) = &body {
            headers.push(rsip::Header::ContentType(content_type.clone()));
        }

        if code == StatusCode::Trying {
            if let Some(ts) = &self.timestamp {
                headers.push(rsip::Header::Timestamp(ts.clone()));
            }
        }

        SipResponse::from(rsip::Response {
            status_code: code,
            headers,
            version: rsip::Version::V2,
            body: body.map(|(_, body)| body).unwrap_or(vec![]),
        })
        .expect("Should be valid response")
    }

    pub fn to_bytes(self) -> bytes::Bytes {
        self.raw.into()
    }
}
