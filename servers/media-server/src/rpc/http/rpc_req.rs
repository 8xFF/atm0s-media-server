use std::fmt::Debug;

use async_std::channel::{bounded, Receiver, Sender};
use cluster::rpc::RpcReqRes;

#[derive(Debug)]
pub struct RpcReqResHttp<P, R> {
    tx: Sender<Result<R, &'static str>>,
    param: P,
}

impl<P, R> RpcReqResHttp<P, R> {
    pub fn new(param: P) -> (Self, Receiver<Result<R, &'static str>>) {
        let (tx, rx) = bounded(1);
        (Self { tx, param }, rx)
    }
}

impl<P: Debug + Send, R: Debug + Send> RpcReqRes<P, R> for RpcReqResHttp<P, R> {
    fn param(&self) -> &P {
        &self.param
    }

    fn answer(&self, res: Result<R, &str>) {
        // self.tx.try_send(res);
        todo!()
    }
}
