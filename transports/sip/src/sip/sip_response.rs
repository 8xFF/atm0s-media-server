use std::fmt::Debug;

use rsip::prelude::ToTypedHeader;

#[derive(Debug)]
pub enum RequiredHeader {
    CallId,
    CSeq,
    From,
    To,
    Via,
}

#[derive(Debug)]
pub enum SipResponseError {
    Missing(RequiredHeader),
}

#[derive(PartialEq, Eq)]
pub struct SipResponse {
    pub raw: rsip::Response,
    pub call_id: rsip::headers::CallId,
    pub cseq: rsip::headers::typed::CSeq,
    pub from: rsip::headers::typed::From,
    pub to: rsip::headers::typed::To,
    pub via: rsip::headers::typed::Via,
    pub timestamp: Option<rsip::headers::Timestamp>,
}

impl Debug for SipResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SipResponse")
            .field("status_code", &self.raw.status_code)
            .field("call_id", &self.call_id)
            .field("cseq", &self.cseq)
            .field("from", &self.from)
            .field("to", &self.to)
            .field("via", &self.via)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

impl SipResponse {
    pub fn from(sip_response: rsip::Response) -> Result<Self, SipResponseError> {
        let mut call_id: Option<rsip::headers::CallId> = None;
        let mut cseq: Option<rsip::headers::typed::CSeq> = None;
        let mut from: Option<rsip::headers::typed::From> = None;
        let mut to: Option<rsip::headers::typed::To> = None;
        let mut via: Option<rsip::headers::typed::Via> = None;
        let mut timestamp: Option<rsip::headers::Timestamp> = None;

        for header in sip_response.headers.iter() {
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
            return Err(SipResponseError::Missing(RequiredHeader::CallId));
        }
        if cseq.is_none() {
            return Err(SipResponseError::Missing(RequiredHeader::CSeq));
        }
        if from.is_none() {
            return Err(SipResponseError::Missing(RequiredHeader::From));
        }
        if to.is_none() {
            return Err(SipResponseError::Missing(RequiredHeader::To));
        }
        if via.is_none() {
            return Err(SipResponseError::Missing(RequiredHeader::Via));
        }

        Ok(Self {
            raw: sip_response,
            call_id: call_id.expect("Must some"),
            cseq: cseq.expect("Must some"),
            from: from.expect("Must some"),
            to: to.expect("Must some"),
            via: via.expect("Must some"),
            timestamp,
        })
    }

    pub fn to_bytes(self) -> bytes::Bytes {
        self.raw.into()
    }
}
