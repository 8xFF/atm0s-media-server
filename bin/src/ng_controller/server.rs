use std::{collections::HashMap, net::SocketAddr};

use media_server_protocol::{
    endpoint::ClusterConnId,
    transport::{
        rtp_engine::{self, RtpReq},
        RpcReq, RpcRes,
    },
};
use tokio::sync::mpsc::Sender;

use crate::http::Rpc;

use super::{
    commands::{NgCmdResult, NgCommand, NgRequest, NgResponse},
    transport::{NgTransport, NgTransportType},
};

pub enum NgControlMsg {
    Request(NgRequest),
    Response(NgResponse),
}

pub struct NgControllerServerConfig {
    pub port: u16,
    pub transport: NgTransportType,
}

pub struct NgControllerServer {
    transport: Box<dyn NgTransport>,
    rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    request_mapper: HashMap<String, SocketAddr>,
}

impl NgControllerServer {
    pub async fn new(config: NgControllerServerConfig, tx: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Self {
        let transport = super::transport::new_transport(config.transport, config.port).await;
        Self {
            transport,
            rpc_sender: tx,
            request_mapper: HashMap::new(),
        }
    }

    pub async fn process(&mut self) {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<NgControlMsg>(32);
        loop {
            tokio::select! {
                Some((req, addr)) = self.transport.recv() => {
                    self.request_mapper.insert(req.id.clone(), addr);
                    tx.send(NgControlMsg::Request(req)).await.unwrap();
                }
                Some(msg) = rx.recv() => {
                    match msg {
                        NgControlMsg::Request(req) => {
                            self.handle_request(req, tx.clone());
                        }
                        NgControlMsg::Response(res) => {
                            if let Some(addr) = self.request_mapper.remove(&res.id) {
                                self.transport.send(res, addr).await;
                            }
                        }
                    }
                }
                else => {
                    break;
                }
            }
        }
    }
}

impl NgControllerServer {
    fn handle_request(&self, req: NgRequest, intenal_sender: Sender<NgControlMsg>) {
        let id = req.id.clone();
        let rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>> = self.rpc_sender.clone();
        tokio::spawn(async move {
            log::info!("[NgControllerServer] Received request: {:?}", req);
            let rpc_req = ng_cmd_to_rpc(req.command);
            let res = match rpc_req {
                Some(req) => {
                    let (rpc, answer) = Rpc::new(req);
                    log::info!("send rpc to main loop for handle ");
                    rpc_sender.send(rpc).await.unwrap();
                    let rpc_res = answer.await;
                    log::info!("got a rpc response");
                    match rpc_res {
                        Ok(res) => NgResponse {
                            id,
                            result: rpc_result_to_ng_res(res).unwrap(),
                        },
                        Err(e) => NgResponse {
                            id,
                            result: NgCmdResult::Error {
                                result: "error".to_string(),
                                error_reason: e.to_string(),
                            },
                        },
                    }
                }
                None => NgResponse {
                    id,
                    result: NgCmdResult::Error {
                        result: "error".to_string(),
                        error_reason: "unsupported command".to_string(),
                    },
                },
            };
            intenal_sender.send(NgControlMsg::Response(res)).await.unwrap();
        });
    }
}

fn ng_cmd_to_rpc(req: NgCommand) -> Option<RpcReq<ClusterConnId>> {
    match req {
        NgCommand::Ping {} => Some(RpcReq::Rtp(RtpReq::Ping)),
        _ => None,
    }
}

fn rpc_result_to_ng_res(res: RpcRes<ClusterConnId>) -> Option<NgCmdResult> {
    match res {
        RpcRes::Rtp(rtp_engine::RtpRes::Ping(Ok(res))) => Some(NgCmdResult::Pong { result: res }),
        _ => None,
    }
}
