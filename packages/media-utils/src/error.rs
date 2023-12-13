use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerError {
    pub code: String,
    pub message: String,
}

impl ServerError {
    pub fn build<T1: ToString, T2: ToString>(code: T1, message: T2) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

pub trait ErrorDebugger {
    fn log_error(&self, msg: &str);
}

impl<D, E: Debug> ErrorDebugger for Result<D, E> {
    fn log_error(&self, msg: &str) {
        if let Err(e) = self {
            log::error!("{}: {:?}", msg, e);
        }
    }
}

pub trait OptionDebugger {
    fn log_option(&self, msg: &str);
}

impl<D> OptionDebugger for Option<D> {
    fn log_option(&self, msg: &str) {
        if self.is_none() {
            log::error!("{}", msg);
        }
    }
}
