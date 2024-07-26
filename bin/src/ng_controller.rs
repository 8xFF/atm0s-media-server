use media_server_protocol::endpoint::ClusterConnId;
use media_server_protocol::transport::{RpcReq, RpcRes};
use media_server_utils::select2;
use req_res::{ng_cmd_to_rpc, rpc_result_to_ng_res};
use rtpengine_ngcontrol::{NgResponse, NgTransport};
use std::net::SocketAddr;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::rpc::Rpc;

mod req_res;

pub enum NgControlMsg {
    Response(NgResponse),
}

pub struct NgControllerServer<T> {
    transport: T,
    rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    answer_tx: Sender<(NgResponse, SocketAddr)>,
    answer_rx: Receiver<(NgResponse, SocketAddr)>,
}

impl<T: NgTransport> NgControllerServer<T> {
    pub fn new(transport: T, rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Self {
        let (answer_tx, answer_rx) = channel(10);
        Self {
            transport,
            rpc_sender,
            answer_tx,
            answer_rx,
        }
    }

    pub async fn recv(&mut self) -> Option<()> {
        match select2::or(self.transport.recv(), self.answer_rx.recv()).await {
            select2::OrOutput::Left(Some((req, remote))) => {
                if let Some(rpc_req) = ng_cmd_to_rpc(req.command) {
                    let (rpc_req, rx) = Rpc::new(rpc_req);
                    let answer_tx = self.answer_tx.clone();
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(res) => {
                                if let Some(result) = rpc_result_to_ng_res(res) {
                                    answer_tx.send((NgResponse { id: req.id, result }, remote)).await;
                                } else {
                                    //TODO send error
                                }
                            }
                            Err(_err) => {
                                //TODO send error
                            }
                        }
                    });
                    self.rpc_sender.send(rpc_req).await.ok()
                } else {
                    Some(())
                }
            }
            select2::OrOutput::Right(Some((res, dest))) => {
                self.transport.send(res, dest).await;
                Some(())
            }
            select2::OrOutput::Left(None) => None,
            select2::OrOutput::Right(None) => None,
        }
    }
}
