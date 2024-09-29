use media_server_protocol::{
    multi_tenancy::AppContext,
    tokens::{RtpEngineToken, WebrtcToken, WhepToken, WhipToken},
};
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "jwt-secure")]
pub mod jwt;

pub trait TokenObject: Serialize + DeserializeOwned {
    fn id() -> &'static str;
}

/// This interface is for validating and generating data in each edge node like media-node
pub trait MediaEdgeSecure {
    fn decode_token<O: TokenObject>(&self, data: &str) -> Option<(AppContext, O)>;
    fn encode_conn_id<C: Serialize + DeserializeOwned>(&self, conn: C, ttl_seconds: u64) -> String;
    fn decode_conn_id<C: Serialize + DeserializeOwned>(&self, data: &str) -> Option<C>;
}

pub trait AppStorage: Send + Sync + 'static {
    fn validate_app(&self, secret: &str) -> Option<AppContext>;
}

/// This interface for generating signed data for gateway, like connect token
pub trait MediaGatewaySecure {
    fn validate_app(&self, token: &str) -> Option<AppContext>;
    fn encode_token<O: TokenObject>(&self, ctx: &AppContext, ob: O, ttl_seconds: u64) -> String;
    fn decode_conn_id<C: Serialize + DeserializeOwned>(&self, data: &str) -> Option<C>;
}

/// This interface for console validate session
pub trait MediaConsoleSecure {
    fn validate_secret(&self, secret: &str) -> bool;
    fn validate_token(&self, token: &str) -> bool;
    fn generate_token(&self) -> String;
}

impl TokenObject for WhipToken {
    fn id() -> &'static str {
        "whip"
    }
}

impl TokenObject for WhepToken {
    fn id() -> &'static str {
        "whep"
    }
}

impl TokenObject for WebrtcToken {
    fn id() -> &'static str {
        "webrtc"
    }
}

impl TokenObject for RtpEngineToken {
    fn id() -> &'static str {
        "rtp"
    }
}
