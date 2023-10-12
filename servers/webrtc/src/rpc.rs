use async_std::channel::{bounded, Receiver, Sender};
use utils::ServerError;

pub(crate) mod http;

pub struct RpcResponse<T> {
    tx: Sender<(u16, Result<T, ServerError>)>,
}

impl<T> RpcResponse<T> {
    pub fn new() -> (Self, Receiver<(u16, Result<T, ServerError>)>) {
        let (tx, rx) = bounded(1);
        (Self { tx }, rx)
    }
    pub fn answer(&mut self, code: u16, res: Result<T, ServerError>) {
        self.tx.send_blocking((code, res));
    }
}

pub struct WhipConnectResponse {
    pub location: String,
    pub sdp: String,
}

pub enum RpcEvent {
    WhipConnect(String, String, RpcResponse<WhipConnectResponse>),
    Connect(String, RpcResponse<String>),
    RemoteIce(u64, String, RpcResponse<()>),
}
