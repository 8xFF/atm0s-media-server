use media_server_protocol::cluster::gen_cluster_session_id;
use media_server_protocol::endpoint::{ClusterConnId, PeerId, RoomId};
use media_server_protocol::transport::{rtpengine, RpcReq, RpcRes};
use media_server_utils::select2;
use rtpengine_ngcontrol::{NgCmdResult, NgCommand, NgRequest, NgResponse, NgTransport};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::rpc::Rpc;

pub struct NgControllerServer<T> {
    transport: T,
    rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    answer_tx: Sender<(String, RpcRes<ClusterConnId>, SocketAddr)>,
    answer_rx: Receiver<(String, RpcRes<ClusterConnId>, SocketAddr)>,
    history: HashMap<String, ClusterConnId>,
}

impl<T: NgTransport> NgControllerServer<T> {
    pub fn new(transport: T, rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Self {
        let (answer_tx, answer_rx) = channel(10);
        Self {
            transport,
            rpc_sender,
            answer_tx,
            answer_rx,
            history: HashMap::new(),
        }
    }

    pub async fn recv(&mut self) -> Option<()> {
        match select2::or(self.transport.recv(), self.answer_rx.recv()).await {
            select2::OrOutput::Left(Some((req, remote))) => self.process_req(req, remote).await,
            select2::OrOutput::Right(Some((id, res, dest))) => self.process_res(id, res, dest).await,
            select2::OrOutput::Left(None) => None,
            select2::OrOutput::Right(None) => None,
        }
    }

    async fn process_req(&mut self, req: NgRequest, remote: SocketAddr) -> Option<()> {
        let rpc_req = match req.command {
            NgCommand::Ping => {
                self.transport.send(req.answer(NgCmdResult::Pong { result: "OK".to_string() }), remote).await;
                return Some(());
            }
            NgCommand::Offer { sdp, call_id, from_tag, ice } => {
                let session_id = gen_cluster_session_id();
                rtpengine::RpcReq::Connect(rtpengine::RtpConnectRequest {
                    call_id: RoomId(call_id),
                    leg_id: PeerId(from_tag),
                    sdp,
                    session_id,
                })
            }
            NgCommand::Answer { sdp, call_id, from_tag, to_tag, ice } => {
                let session_id = gen_cluster_session_id();
                rtpengine::RpcReq::Connect(rtpengine::RtpConnectRequest {
                    call_id: RoomId(call_id),
                    leg_id: PeerId(to_tag),
                    sdp,
                    session_id,
                })
            }
            NgCommand::Delete { ref from_tag, .. } => {
                if let Some(conn) = self.history.get(from_tag) {
                    rtpengine::RpcReq::Delete(conn.clone())
                } else {
                    self.transport
                        .send(
                            req.answer(NgCmdResult::Error {
                                error_reason: "NOT_FOUND".to_string(),
                                result: "Not found".to_string(),
                            }),
                            remote,
                        )
                        .await;
                    return Some(());
                }
            }
        };

        let (rpc_req, rx) = Rpc::new(RpcReq::RtpEngine(rpc_req));
        let answer_tx = self.answer_tx.clone();
        let req_id = req.id.clone();
        tokio::spawn(async move {
            match rx.await {
                Ok(res) => {
                    if let Err(e) = answer_tx.send((req_id, res, remote)).await {
                        log::error!("[NgControllerServer] send answer to main task error {e:?}");
                    }
                }
                Err(_err) => {
                    //TODO send error
                }
            }
        });
        self.rpc_sender.send(rpc_req).await.ok()
    }

    async fn process_res(&mut self, id: String, res: RpcRes<ClusterConnId>, dest: SocketAddr) -> Option<()> {
        let result = match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::Connect(Ok((peer, conn, sdp)))) => {
                self.history.insert(peer.0, conn);
                NgCmdResult::Answer {
                    result: "ok".to_string(),
                    sdp: Some(sdp),
                }
            }
            RpcRes::RtpEngine(rtpengine::RpcRes::Delete(Ok(peer_id))) => {
                self.history.remove(&peer_id.0);
                NgCmdResult::Delete { result: "ok".to_string() }
            }
            RpcRes::RtpEngine(rtpengine::RpcRes::Connect(Err(res))) | RpcRes::RtpEngine(rtpengine::RpcRes::Delete(Err(res))) => NgCmdResult::Error {
                result: res.code.to_string(),
                error_reason: res.message,
            },
            _ => {
                return Some(());
            }
        };
        self.transport.send(NgResponse { id, result }, dest).await;
        Some(())
    }
}
