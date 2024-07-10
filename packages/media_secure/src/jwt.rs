use crate::{MediaConsoleSecure, MediaEdgeSecure, MediaGatewaySecure};
use jwt_simple::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

const CONN_ID_TYPE: &str = "conn";
const CONSOLE_SESSION_TYPE: &str = "console_session";

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
        let options = VerificationOptions {
            allowed_issuers: Some(HashSet::from_strings(&[_type])),
            ..Default::default()
        };
        let claims = self.key.verify_token::<O>(token, Some(options)).ok()?;
        if let Some(expires_at) = claims.expires_at {
            let now = Clock::now_since_epoch();
            if now >= expires_at {
                return None;
            }
        }
        Some(claims.custom)
    }

    fn encode_conn_id<C: Serialize + DeserializeOwned>(&self, conn: C, ttl_seconds: u64) -> String {
        let claims = Claims::with_custom_claims(conn, Duration::from_secs(ttl_seconds)).with_issuer(CONN_ID_TYPE);
        self.key.authenticate(claims).expect("Should create jwt")
    }

    fn decode_conn_id<C: Serialize + DeserializeOwned>(&self, token: &str) -> Option<C> {
        let options = VerificationOptions {
            allowed_issuers: Some(HashSet::from_strings(&[CONN_ID_TYPE])),
            ..Default::default()
        };
        let claims = self.key.verify_token::<C>(token, Some(options)).ok()?;
        if let Some(expires_at) = claims.expires_at {
            let now = Clock::now_since_epoch();
            if now >= expires_at {
                return None;
            }
        }
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

    fn decode_conn_id<C: Serialize + DeserializeOwned>(&self, token: &str) -> Option<C> {
        let options = VerificationOptions {
            allowed_issuers: Some(HashSet::from_strings(&[CONN_ID_TYPE])),
            ..Default::default()
        };
        let claims = self.key.verify_token::<C>(token, Some(options)).ok()?;
        if let Some(expires_at) = claims.expires_at {
            let now = Clock::now_since_epoch();
            if now >= expires_at {
                return None;
            }
        }
        Some(claims.custom)
    }
}

#[derive(Clone)]
pub struct MediaConsoleSecureJwt {
    key_str: String,
    key: HS256Key,
}

impl From<&[u8]> for MediaConsoleSecureJwt {
    fn from(key: &[u8]) -> Self {
        Self {
            key: HS256Key::from_bytes(key),
            key_str: String::from_utf8_lossy(key).to_string(),
        }
    }
}

impl MediaConsoleSecure for MediaConsoleSecureJwt {
    fn validate_secert(&self, secret: &str) -> bool {
        self.key_str.eq(secret)
    }

    fn validate_token(&self, token: &str) -> bool {
        let options = VerificationOptions {
            allowed_issuers: Some(HashSet::from_strings(&[CONSOLE_SESSION_TYPE])),
            ..Default::default()
        };
        self.key.verify_token::<()>(token, Some(options)).is_ok()
    }

    fn generate_token(&self) -> String {
        let claims = Claims::with_custom_claims((), Duration::from_secs(10000)).with_issuer(CONSOLE_SESSION_TYPE);
        self.key.authenticate(claims).expect("Should create jwt")
    }
}

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use serde::{Deserialize, Serialize};

    use crate::{
        jwt::{MediaEdgeSecureJwt, MediaGatewaySecureJwt},
        MediaEdgeSecure, MediaGatewaySecure,
    };

    #[test]
    fn object_test() {
        #[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
        struct Test {
            value: u8,
        }

        let secure_key = b"12345678";

        let gateway_jwt = MediaGatewaySecureJwt::from(secure_key.as_slice());
        let edge_jwt = MediaEdgeSecureJwt::from(secure_key.as_slice());

        let ob = Test { value: 1 };
        let token = gateway_jwt.encode_obj("test_type", ob.clone(), 1);

        //if wrong _type should error
        assert_eq!(edge_jwt.decode_obj::<Test>("wrong_type", &token), None, "Should error if wrong type");
        assert_eq!(edge_jwt.decode_obj::<Test>("test_type", &token), Some(ob), "Should decode ok");

        // it should error after timeout 1s
        sleep(Duration::from_millis(1300));
        assert_eq!(edge_jwt.decode_obj::<Test>("test_type", &token), None, "Should error after timeout");
    }

    #[test]
    fn conn_test() {
        #[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
        struct Test {
            value: u8,
        }

        let secure_key = b"12345678";

        let gateway_jwt = MediaGatewaySecureJwt::from(secure_key.as_slice());
        let edge_jwt = MediaEdgeSecureJwt::from(secure_key.as_slice());

        let ob = Test { value: 1 };
        let token = edge_jwt.encode_conn_id(ob.clone(), 1);

        assert_eq!(edge_jwt.decode_conn_id::<Test>(&token), Some(ob.clone()), "Should decode ok");
        assert_eq!(gateway_jwt.decode_conn_id::<Test>(&token), Some(ob), "Should decode ok");

        // it should error after timeout 1s
        sleep(Duration::from_millis(1300));
        assert_eq!(edge_jwt.decode_conn_id::<Test>(&token), None, "Should error after timeout");
        assert_eq!(gateway_jwt.decode_conn_id::<Test>(&token), None, "Should error after timeout");
    }
}
