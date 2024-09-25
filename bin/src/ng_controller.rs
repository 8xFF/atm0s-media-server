use media_server_protocol::cluster::gen_cluster_session_id;
use media_server_protocol::endpoint::{ClusterConnId, PeerId, RoomId};
use media_server_protocol::tokens::{RtpEngineToken, RTPENGINE_TOKEN};
use media_server_protocol::transport::{rtpengine, RpcReq, RpcRes};
use media_server_secure::MediaEdgeSecure;
use media_server_utils::select2;
use rtpengine_ngcontrol::{NgCmdResult, NgCommand, NgRequest, NgResponse, NgTransport};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::rpc::Rpc;

pub struct NgControllerServer<T, S> {
    transport: T,
    secure: Arc<S>,
    rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    answer_tx: Sender<(String, RpcRes<ClusterConnId>, SocketAddr)>,
    answer_rx: Receiver<(String, RpcRes<ClusterConnId>, SocketAddr)>,
}

impl<T: NgTransport, S: 'static + MediaEdgeSecure> NgControllerServer<T, S> {
    pub fn new(transport: T, secure: Arc<S>, rpc_sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Self {
        let (answer_tx, answer_rx) = channel(10);
        Self {
            transport,
            secure,
            rpc_sender,
            answer_tx,
            answer_rx,
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
                self.transport.send(req.answer(NgCmdResult::Pong { result: "ok".to_string() }), remote).await;
                return Some(());
            }
            NgCommand::Offer { ref sdp, ref atm0s_token, .. } | NgCommand::Answer { ref sdp, ref atm0s_token, .. } => {
                if let Some(token) = self.secure.decode_obj::<RtpEngineToken>(RTPENGINE_TOKEN, atm0s_token) {
                    let session_id = gen_cluster_session_id();
                    rtpengine::RpcReq::CreateAnswer(rtpengine::RtpCreateAnswerRequest {
                        app: token.app.unwrap_or_default().into(),
                        session_id,
                        room: RoomId(token.room),
                        peer: PeerId(token.peer),
                        sdp: sdp.clone(),
                        record: token.record,
                        extra_data: token.extra_data,
                    })
                } else {
                    self.send_err(&req, "TOKEN_FAILED", "Token parse error", remote).await;
                    return Some(());
                }
            }
            NgCommand::Delete { ref conn_id, .. } => {
                if let Ok(conn) = ClusterConnId::from_str(conn_id) {
                    rtpengine::RpcReq::Delete(conn)
                } else {
                    self.send_err(&req, "NOT_FOUND", "Connection parse error", remote).await;
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
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateAnswer(Ok((conn, sdp)))) => NgCmdResult::Answer {
                result: "ok".to_string(),
                conn: Some(conn.to_string()),
                sdp: Some(sdp),
            },
            RpcRes::RtpEngine(rtpengine::RpcRes::Delete(Ok(_conn))) => NgCmdResult::Delete { result: "ok".to_string() },
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateAnswer(Err(res))) | RpcRes::RtpEngine(rtpengine::RpcRes::Delete(Err(res))) => NgCmdResult::Error {
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

    async fn send_err(&self, req: &NgRequest, result: &str, err: &str, remote: SocketAddr) {
        self.transport
            .send(
                req.answer(NgCmdResult::Error {
                    error_reason: err.to_string(),
                    result: result.to_string(),
                }),
                remote,
            )
            .await;
    }
}
