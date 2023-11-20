use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::{rpc::RpcEvent, server::webrtc_session::run_webrtc_endpoint};

use self::rtmp_session::RtmpSession;
use async_std::{channel::Sender, prelude::FutureExt};
use cluster::{Cluster, ClusterEndpoint};
use media_utils::{EndpointSubscribeScope, ErrorDebugger, ServerError, Timer};
use parking_lot::RwLock;
use transport::RpcResponse;
use transport_webrtc::{
    SdkTransportLifeCycle, SdpBoxRewriteScope, WebrtcConnectRequestSender, WebrtcConnectResponse, WebrtcRemoteIceRequest, WhepConnectResponse, WhepTransportLifeCycle, WhipConnectResponse,
    WhipTransportLifeCycle,
};

mod rtmp_session;
mod webrtc_session;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct PeerIdentity {
    room: String,
    peer: String,
}

impl PeerIdentity {
    pub fn new(room: &str, peer: &str) -> Self {
        Self { room: room.into(), peer: peer.into() }
    }
}

pub enum InternalControl {
    RemoteIce(WebrtcRemoteIceRequest, RpcResponse<()>),
    ForceClose(Sender<()>),
}

pub struct MediaServer<C, CR> {
    _tmp_cr: std::marker::PhantomData<CR>,
    cluster: C,
    counter: u64,
    conns: Arc<RwLock<HashMap<String, Sender<InternalControl>>>>,
    peers: Arc<RwLock<HashMap<PeerIdentity, Sender<InternalControl>>>>,
    timer: Arc<dyn Timer>,
}

impl<C, CR: 'static> MediaServer<C, CR>
where
    C: Cluster<CR> + Send + Sync + 'static,
    CR: ClusterEndpoint + Send + Sync + 'static,
{
    pub fn new(cluster: C) -> Self {
        Self {
            _tmp_cr: std::marker::PhantomData,
            cluster,
            counter: 0,
            conns: Arc::new(RwLock::new(HashMap::new())),
            peers: Arc::new(RwLock::new(HashMap::new())),
            timer: Arc::new(media_utils::SystemTimer()),
        }
    }

    pub async fn on_incomming(&mut self, event: RpcEvent) {
        let peers = self.peers.clone();
        let conns = self.conns.clone();

        match event {
            RpcEvent::WhipConnect(token, sdp, mut res) => {
                //TODO validate token to get room
                let room = token;
                let peer = "publisher";
                log::info!("[MediaServer] on whip connection from {} {}", room, peer);
                let senders = vec![
                    WebrtcConnectRequestSender {
                        kind: "audio".to_string(),
                        name: "audio_main".to_string(),
                        label: "audio_main".to_string(),
                        uuid: "audio_main".to_string(),
                        screen: None,
                    },
                    WebrtcConnectRequestSender {
                        kind: "video".to_string(),
                        name: "video_main".to_string(),
                        label: "video_main".to_string(),
                        uuid: "video_main".to_string(),
                        screen: None,
                    },
                ];
                let life_cycle = WhipTransportLifeCycle::new(self.timer.now_ms());
                match run_webrtc_endpoint(
                    &mut self.counter,
                    conns,
                    peers,
                    &mut self.cluster,
                    life_cycle,
                    EndpointSubscribeScope::RoomManual,
                    &room,
                    peer,
                    &sdp,
                    senders,
                    None,
                )
                .await
                {
                    Ok((sdp, conn_id)) => {
                        res.answer(
                            200,
                            Ok(WhipConnectResponse {
                                location: format!("/api/whip/conn/{}", conn_id),
                                sdp,
                            }),
                        );
                    }
                    Err(err) => {
                        res.answer(200, Err(err));
                    }
                }
            }
            RpcEvent::WhipPatch(_conn_id, sdp, mut res) => {
                res.answer(200, Err(ServerError::build("NOT_IMPLEMENTED", "Not implemented")));
            }
            RpcEvent::WhipClose(conn_id, mut res) => {
                if let Some(old_tx) = conns.write().remove(&conn_id) {
                    async_std::task::spawn(async move {
                        let (tx, rx) = async_std::channel::bounded(1);
                        old_tx.send(InternalControl::ForceClose(tx.clone())).await.log_error("need send");
                        if let Ok(e) = rx.recv().timeout(Duration::from_secs(1)).await {
                            let control_res = e.map_err(|_e| ServerError::build("INTERNAL_QUEUE_ERROR", "Internal queue error"));
                            res.answer(200, control_res);
                        } else {
                            res.answer(503, Err(ServerError::build("REQUEST_TIMEOUT", "Request timeout")));
                        }
                    });
                } else {
                    res.answer(404, Err(ServerError::build("NOT_FOUND", "Connnection not found")));
                }
            }
            RpcEvent::WhepConnect(token, sdp, mut res) => {
                //TODO validate token to get room
                let room = token;
                let peer = format!("whep-{}", self.counter);
                log::info!("[MediaServer] on whep connection from {} {}", room, peer);
                let life_cycle = WhepTransportLifeCycle::new(self.timer.now_ms());
                match run_webrtc_endpoint(
                    &mut self.counter,
                    conns,
                    peers,
                    &mut self.cluster,
                    life_cycle,
                    EndpointSubscribeScope::RoomAuto,
                    &room,
                    &peer,
                    &sdp,
                    vec![],
                    Some(SdpBoxRewriteScope::OnlyTrack),
                )
                .await
                {
                    Ok((sdp, conn_id)) => {
                        res.answer(
                            200,
                            Ok(WhepConnectResponse {
                                location: format!("/api/whep/conn/{}", conn_id),
                                sdp,
                            }),
                        );
                    }
                    Err(err) => {
                        res.answer(200, Err(err));
                    }
                }
            }
            RpcEvent::WhepPatch(_conn_id, _sdp, mut res) => {
                res.answer(200, Err(ServerError::build("NOT_IMPLEMENTED", "Not implemented")));
            }
            RpcEvent::WhepClose(conn_id, mut res) => {
                if let Some(old_tx) = conns.write().remove(&conn_id) {
                    async_std::task::spawn(async move {
                        let (tx, rx) = async_std::channel::bounded(1);
                        old_tx.send(InternalControl::ForceClose(tx.clone())).await.log_error("need send");
                        if let Ok(e) = rx.recv().timeout(Duration::from_secs(1)).await {
                            let control_res = e.map_err(|_e| ServerError::build("INTERNAL_QUEUE_ERROR", "Internal queue error"));
                            res.answer(200, control_res);
                        } else {
                            res.answer(503, Err(ServerError::build("REQUEST_TIMEOUT", "Request timeout")));
                        }
                    });
                } else {
                    res.answer(404, Err(ServerError::build("NOT_FOUND", "Connnection not found")));
                }
            }
            RpcEvent::WebrtcConnect(req, mut res) => {
                log::info!("[MediaServer] on webrtc connection from {} {}", req.room, req.peer);
                let sub_scope = req.sub_scope.unwrap_or(EndpointSubscribeScope::RoomAuto);
                let life_cycle = SdkTransportLifeCycle::new(self.timer.now_ms());
                match run_webrtc_endpoint(
                    &mut self.counter,
                    conns,
                    peers,
                    &mut self.cluster,
                    life_cycle,
                    sub_scope,
                    &req.room,
                    &req.peer,
                    &req.sdp,
                    req.senders,
                    Some(SdpBoxRewriteScope::StreamAndTrack),
                )
                .await
                {
                    Ok((sdp, conn_id)) => {
                        res.answer(200, Ok(WebrtcConnectResponse { conn_id, sdp }));
                    }
                    Err(err) => {
                        res.answer(200, Err(err));
                    }
                }
            }
            RpcEvent::WebrtcRemoteIce(req, res) => {
                if let Some(tx) = self.conns.read().get(&req.conn_id) {
                    if let Err(_e) = tx.send_blocking(InternalControl::RemoteIce(req, res)) {
                        //TODO handle this
                    };
                }
            }
            RpcEvent::RtmpConnect(transport, room_id, peer_id) => {
                log::info!("[MediaServer] on rtmp connection from {} {}", room_id, peer_id);
                let mut session = match RtmpSession::new(&room_id, &peer_id, &mut self.cluster, transport).await {
                    Ok(res) => res,
                    Err(e) => {
                        log::error!("Error on create rtmp session: {:?}", e);
                        return;
                    }
                };

                async_std::task::spawn(async move {
                    log::info!("[MediaServer] start loop for rtmp endpoint");
                    while let Some(_) = session.recv().await {}
                    log::info!("[MediaServer] stop loop for rtmp endpoint");
                });
            }
        }
    }
}
