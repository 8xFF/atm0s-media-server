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
