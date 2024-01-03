use proc_macro::{IntoVecU8, TryFromSliceU8};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, IntoVecU8, TryFromSliceU8)]
pub struct MediaEndpointLogResponse {}
