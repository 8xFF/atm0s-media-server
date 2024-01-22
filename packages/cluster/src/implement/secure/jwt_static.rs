use crate::{MediaSessionToken, SessionTokenSigner, SessionTokenVerifier};
use hmac::{Hmac, Mac};
use jwt::{SignWithKey, VerifyWithKey};
use sha2::Sha256;

pub struct JwtStaticToken {
    key: Hmac<Sha256>,
}

impl JwtStaticToken {
    pub fn new(secret: &str) -> Self {
        Self {
            key: Hmac::new_from_slice(secret.as_bytes()).expect("Should create HMAC key"),
        }
    }
}

impl SessionTokenSigner for JwtStaticToken {
    fn sign_media_session(&self, token: &MediaSessionToken) -> String {
        token.sign_with_key(&self.key).expect("Should sign media session")
    }
    fn sign_conn_id(&self, conn_id: &crate::MediaConnId) -> String {
        conn_id.sign_with_key(&self.key).expect("Should sign media conn_id")
    }
}

impl SessionTokenVerifier for JwtStaticToken {
    fn verify_media_session(&self, token: &str) -> Option<MediaSessionToken> {
        token.verify_with_key(&self.key).ok()
    }
    fn verify_conn_id(&self, token: &str) -> Option<crate::MediaConnId> {
        token.verify_with_key(&self.key).ok()
    }
}
