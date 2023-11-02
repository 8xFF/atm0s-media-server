use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::rpc::RpcEvent;

use self::{rtmp_session::RtmpSession, webrtc_session::WebrtcSession};
use async_std::{channel::Sender, prelude::FutureExt};
use cluster::{Cluster, ClusterEndpoint};
use media_utils::{EndpointSubscribeScope, ServerError, Timer};
use parking_lot::RwLock;
use transport::RpcResponse;
use transport_webrtc::{WebrtcConnectResponse, WebrtcRemoteIceRequest};

mod rtmp_session;
mod webrtc_session;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct PeerIdentity {
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
    C: Cluster<CR>,
    CR: ClusterEndpoint,
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
        match event {
            RpcEvent::WhipConnect(_token, _sdp, mut res) => {
                res.answer(200, Err(ServerError::build("NOT_IMPLEMENTED", "Not implemented now")));
            }
            RpcEvent::WebrtcConnect(req, mut res) => {
                log::info!("[MediaServer] on webrtc connection from {} {}", req.room, req.peer);
                let sub_scope = req.sub_scope.unwrap_or(EndpointSubscribeScope::RoomAuto);
                let (mut session, tx, answer_sdp) = match WebrtcSession::new(&req.room, &req.peer, sub_scope, &mut self.cluster, &req.sdp, req.senders, self.timer.now_ms()).await {
                    Ok(res) => res,
                    Err(e) => {
                        log::error!("Error on create webrtc session: {:?}", e);
                        res.answer(200, Err(ServerError::build("CONNECT_ERROR", format!("{:?}", e))));
                        return;
                    }
                };

                let connect_id = format!("conn-{}", self.counter);
                self.counter += 1;
                let local_peer_id = PeerIdentity::new(&req.room, &req.peer);

                res.answer_async(
                    200,
                    Ok(WebrtcConnectResponse {
                        sdp: answer_sdp,
                        conn_id: connect_id.clone(),
                    }),
                )
                .await;

                let peers_c = self.peers.clone();
                let conns_c = self.conns.clone();

                conns_c.write().insert(connect_id.clone(), tx.clone());
                if let Some(old_tx) = peers_c.write().insert(local_peer_id.clone(), tx) {
                    let (tx, rx) = async_std::channel::bounded(1);
                    old_tx.send(InternalControl::ForceClose(tx.clone())).await;
                    rx.recv().timeout(Duration::from_secs(1)).await;
                }

                async_std::task::spawn(async move {
                    log::info!("[MediaServer] start loop for webrtc endpoint");
                    while let Some(_) = session.recv().await {}
                    log::info!("[MediaServer] stop loop for webrtc endpoint");
                    conns_c.write().remove(&connect_id);
                    peers_c.write().remove(&local_peer_id);
                });
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
