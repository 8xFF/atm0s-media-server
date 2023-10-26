use std::{collections::HashMap, sync::Arc, time::Duration};

use self::webrtc_session::WebrtcSession;
use crate::rpc::{RpcEvent, RpcResponse, WebrtcConnectResponse};
use async_std::{channel::Sender, prelude::FutureExt};
use cluster::{Cluster, ClusterEndpoint};
use parking_lot::RwLock;
use utils::{EndpointSubscribeScope, ServerError, Timer};

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
    RemoteIce(String, RpcResponse<()>),
    ForceClose(Sender<()>),
}

pub struct WebrtcServer<C, CR> {
    _tmp_cr: std::marker::PhantomData<CR>,
    cluster: C,
    counter: u64,
    conns: Arc<RwLock<HashMap<String, Sender<InternalControl>>>>,
    peers: Arc<RwLock<HashMap<PeerIdentity, Sender<InternalControl>>>>,
    timer: Arc<dyn Timer>,
}

impl<C, CR: 'static> WebrtcServer<C, CR>
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
            timer: Arc::new(utils::SystemTimer()),
        }
    }

    pub async fn on_incomming(&mut self, event: RpcEvent) {
        match event {
            RpcEvent::WhipConnect(_token, sdp, mut res) => {
                res.answer(200, Err(ServerError::build("NOT_IMPLEMENTED", "Not implemented now")));
            }
            RpcEvent::WebrtcConnect(req, mut res) => {
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
                    log::info!("[WebrtcServer] start loop for endpoint");
                    while let Some(_) = session.recv().await {}
                    log::info!("[WebrtcServer] stop loop for endpoint");
                    conns_c.write().remove(&connect_id);
                    peers_c.write().remove(&local_peer_id);
                });
            }
            RpcEvent::WebrtcRemoteIce(conn_id, ice, res) => {
                if let Some(tx) = self.conns.read().get(&conn_id) {
                    if let Err(_e) = tx.send_blocking(InternalControl::RemoteIce(ice, res)) {
                        //TODO handle this
                    };
                }
            }
        }
    }
}
