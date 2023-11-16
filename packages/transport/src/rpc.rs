use std::fmt::Debug;

use async_std::channel::{bounded, Receiver, Sender};
use media_utils::ServerError;

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
#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn test_rpc_response() {
        let (mut rpc_response, rx) = RpcResponse::<i32>::new();
        rpc_response.answer(200, Ok(42));
        let (code, res) = rx.recv().await.unwrap();
        assert_eq!(code, 200);
        assert_eq!(res.unwrap(), 42);

        rpc_response.answer(500, Err(ServerError::build(500, "Internal Server Error")));
        let (code, res) = rx.recv().await.unwrap();
        assert_eq!(code, 500);
        assert_eq!(
            res.unwrap_err(),
            ServerError {
                code: "500".to_string(),
                message: "Internal Server Error".to_string()
            }
        );

        rpc_response.answer_async(200, Ok(42)).await;
        let (code, _) = rx.recv().await.unwrap();
        assert_eq!(code, 200);
    }
}
