use poem_openapi::types::{ToJSON, Type};
use poem_openapi::{types::ParseFromJSON, Object};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Object)]
pub struct Response<T: ParseFromJSON + ToJSON + Type + Send + Sync> {
    pub status: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}
