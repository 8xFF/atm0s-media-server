use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "jwt-secure")]
pub mod jwt;

/// This interface is for validating and generating data in each edge node like media-node
pub trait MediaEdgeSecure {
    fn decode_obj<O: Serialize + DeserializeOwned>(&self, _type: &'static str, data: &str) -> Option<O>;
    fn encode_conn_id<C: Serialize + DeserializeOwned>(&self, conn: C, ttl_ms: u64) -> String;
    fn decode_conn_id<C: Serialize + DeserializeOwned>(&self, data: &str) -> Option<C>;
}

/// This interface for generate signed data for gateway, like connect token
pub trait MediaGatewaySecure {
    fn validate_app(&self, token: &str) -> bool;
    fn encode_obj<O: Serialize + DeserializeOwned>(&self, _type: &'static str, ob: O, ttl_ms: u64) -> String;
}