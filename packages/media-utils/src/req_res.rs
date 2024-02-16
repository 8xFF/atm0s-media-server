use poem_openapi::{
    types::{ParseFromJSON, ToJSON, Type},
    Object,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct Response<T: ParseFromJSON + ToJSON + Type + Send + Sync, E: ParseFromJSON + ToJSON + Type + Send + Sync> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[oai(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<E>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[oai(skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[oai(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: ParseFromJSON + ToJSON + Type + Send + Sync, E: ParseFromJSON + ToJSON + Type + Send + Sync> Response<T, E> {
    pub fn success<T2: Into<T>>(data: T2) -> Self {
        Self {
            success: true,
            data: Some(data.into()),
            error_code: None,
            error_msg: None,
        }
    }

    pub fn error<E2: Into<E>>(error_code: E2, error_msg: &str) -> Self {
        Self {
            success: false,
            error_code: Some(error_code.into()),
            error_msg: Some(error_msg.to_string()),
            data: None,
        }
    }
}
