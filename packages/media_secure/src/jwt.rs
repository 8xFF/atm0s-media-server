use crate::{MediaEdgeSecure, MediaGatewaySecure};
use jwt_simple::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

const HEADER_CONN: &'static str = "C";

pub struct MediaEdgeSecureJwt {
    key: HS256Key,
}

impl From<&[u8]> for MediaEdgeSecureJwt {
    fn from(key: &[u8]) -> Self {
        Self { key: HS256Key::from_bytes(key) }
    }
}

impl MediaEdgeSecure for MediaEdgeSecureJwt {
    fn decode_obj<O: Serialize + DeserializeOwned>(&self, _type: &'static str, token: &str) -> Option<O> {
        let mut options = VerificationOptions::default();
        options.allowed_issuers = Some(HashSet::from_strings(&[_type]));
        let claims = self.key.verify_token::<O>(token, Some(options)).ok()?;
        Some(claims.custom)
    }

    fn encode_conn_id<C: Serialize + DeserializeOwned>(&self, conn: C, ttl_seconds: u64) -> String {
        let claims = Claims::with_custom_claims(conn, Duration::from_secs(ttl_seconds)).with_issuer("conn");
        self.key.authenticate(claims).expect("Should create jwt")
    }

    fn decode_conn_id<C: Serialize + DeserializeOwned>(&self, token: &str) -> Option<C> {
        let mut options = VerificationOptions::default();
        options.allowed_issuers = Some(HashSet::from_strings(&["conn"]));
        let claims = self.key.verify_token::<C>(token, Some(options)).ok()?;
        Some(claims.custom)
    }
}

pub struct MediaGatewaySecureJwt {
    key_str: String,
    key: HS256Key,
}

impl From<&[u8]> for MediaGatewaySecureJwt {
    fn from(key: &[u8]) -> Self {
        Self {
            key_str: String::from_utf8_lossy(key).to_string(),
            key: HS256Key::from_bytes(key),
        }
    }
}

impl MediaGatewaySecure for MediaGatewaySecureJwt {
    fn validate_app(&self, token: &str) -> bool {
        self.key_str.eq(token)
    }

    fn encode_obj<O: Serialize + DeserializeOwned>(&self, _type: &'static str, ob: O, ttl_seconds: u64) -> String {
        let claims = Claims::with_custom_claims(ob, Duration::from_secs(ttl_seconds)).with_issuer(_type);
        self.key.authenticate(claims).expect("Should create jwt")
    }
}
