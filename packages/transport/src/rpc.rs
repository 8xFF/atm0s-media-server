use std::fmt::Debug;

use async_std::channel::{bounded, Receiver, Sender};
use utils::ServerError;

#[derive(Clone)]
pub struct RpcResponse<T> {
    tx: Sender<(u16, Result<T, ServerError>)>,
}

impl<T> Debug for RpcResponse<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcResponse").finish()
    }
}

impl<T> Eq for RpcResponse<T> {}

impl<T> PartialEq for RpcResponse<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> RpcResponse<T> {
    pub fn new() -> (Self, Receiver<(u16, Result<T, ServerError>)>) {
        let (tx, rx) = bounded(1);
        (Self { tx }, rx)
    }
    pub fn answer(&mut self, code: u16, res: Result<T, ServerError>) {
        self.tx.send_blocking((code, res));
    }

    pub async fn answer_async(&mut self, code: u16, res: Result<T, ServerError>) {
        self.tx.send((code, res)).await;
    }
}
