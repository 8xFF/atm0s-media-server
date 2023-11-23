use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use rsip::headers::CallId;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct CallId2(pub String);

impl From<&str> for CallId2 {
    fn from(s: &str) -> Self {
        CallId2(s.to_string())
    }
}

impl From<String> for CallId2 {
    fn from(s: String) -> Self {
        CallId2(s)
    }
}

impl From<CallId2> for String {
    fn from(call_id: CallId2) -> Self {
        call_id.0
    }
}

impl From<&CallId2> for String {
    fn from(call_id: &CallId2) -> Self {
        call_id.0.clone()
    }
}

impl CallId2 {
    pub fn new() -> Self {
        Self(generate_random_string(16))
    }
}

impl From<CallId> for CallId2 {
    fn from(call_id: CallId) -> Self {
        Self(call_id.to_string())
    }
}

pub fn generate_random_string(length: usize) -> String {
    let rng = thread_rng();
    let random_string: String = rng.sample_iter(&Alphanumeric).take(length).map(char::from).collect();

    random_string
}
