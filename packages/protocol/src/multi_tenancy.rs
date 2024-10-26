use std::fmt::Display;

use derive_more::derive::{AsRef, Deref, Display, From, Into};
use serde::{Deserialize, Serialize};

use crate::protobuf;

#[derive(From, Into, AsRef, Deref, Debug, Display, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppId(String);

impl AppId {
    pub fn root_app() -> Self {
        AppId("".to_string())
    }
}

impl From<&str> for AppId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(From, Deref, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct AppSecret(String);

impl From<&str> for AppSecret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppContext {
    pub app: AppId,
}

impl AppContext {
    pub fn root_app() -> Self {
        Self { app: AppId::root_app() }
    }
}

impl Display for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.app.is_empty() {
            f.write_fmt(format_args!("App(ROOT)"))
        } else {
            f.write_fmt(format_args!("App(\"{}\")", self.app))
        }
    }
}

impl From<protobuf::shared::AppContext> for AppContext {
    fn from(value: protobuf::shared::AppContext) -> Self {
        Self {
            app: value.app.unwrap_or_default().into(),
        }
    }
}

impl From<Option<protobuf::shared::AppContext>> for AppContext {
    fn from(value: Option<protobuf::shared::AppContext>) -> Self {
        Self {
            app: value.and_then(|v| v.app.map(|a| a.into())).unwrap_or_else(AppId::root_app),
        }
    }
}

impl From<AppContext> for protobuf::shared::AppContext {
    fn from(value: AppContext) -> Self {
        Self { app: Some(value.app.into()) }
    }
}
