use std::{collections::HashMap, sync::Arc, time::Duration};

use async_std::{
    channel::{bounded, Receiver, Sender},
    prelude::FutureExt as _,
};
use cluster::{Cluster, ClusterEndpoint};
use endpoint::{BitrateLimiterType, MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use futures::{select, FutureExt};
use media_utils::{EndpointSubscribeScope, ServerError};
use parking_lot::RwLock;
use transport_webrtc::{SdpBoxRewriteScope, TransportLifeCycle, WebrtcConnectRequestSender, WebrtcTransport, WebrtcTransportEvent};

use super::{InternalControl, PeerIdentity};

#[derive(Debug)]
pub enum WebrtcSessionError {
    PreconditionError,
    NetworkError,
    SdpError,
}

pub struct WebrtcSession<E: ClusterEndpoint, L: TransportLifeCycle> {
    endpoint: MediaEndpoint<WebrtcTransport<L>, WebrtcTransportEvent, E>,
    rx: Receiver<InternalControl>,
}

impl<E: ClusterEndpoint, L: TransportLifeCycle> WebrtcSession<E, L> {
    pub async fn new<C: Cluster<E>>(
        room: &str,
        peer: &str,
        sub_scope: EndpointSubscribeScope,
        bitrate_type: BitrateLimiterType,
        life_cycle: L,
        cluster: &mut C,
        sdp: &str,
        senders: Vec<WebrtcConnectRequestSender>,
        sdp_rewrite: Option<SdpBoxRewriteScope>,
    ) -> Result<(Self, Sender<InternalControl>, String), WebrtcSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(room, peer, sub_scope, bitrate_type);
        endpoint_pre.check().map_err(|_e| WebrtcSessionError::PreconditionError)?;
        let room = cluster.build(room, peer);
        let mut transport = WebrtcTransport::new(life_cycle, sdp_rewrite).await.map_err(|_| WebrtcSessionError::NetworkError)?;
        for sender in senders {
            transport.map_remote_stream(sender);
        }
        let answer = transport.on_remote_sdp(sdp).map_err(|_| WebrtcSessionError::SdpError)?;
        let endpoint = endpoint_pre.build(transport, room);
        let (tx, rx) = bounded(10);

        Ok((Self { endpoint, rx }, tx, answer))
    }

    pub async fn recv(&mut self) -> Option<()> {
        select! {
            e = self.endpoint.recv().fuse() => match e {
                Ok(e) => {
                    match e {
                        MediaEndpointOutput::Continue => {}
                        MediaEndpointOutput::ConnectionClosed => {
                            log::info!("Connection closed");
                            return None;
                        }
                        MediaEndpointOutput::ConnectionCloseRequest => {
                            log::info!("Connection close request");
                            self.endpoint.close().await;
                            return None;
                        }
                    }
                    Some(())
                },
                Err(e) => {
                    log::error!("Error on endpoint recv: {:?}", e);
                    None
                }
            },
            e = self.rx.recv().fuse() => match e {
                Ok(InternalControl::RemoteIce(ice, mut res)) => {
                    if let Err(err) = self.endpoint.on_custom_event(WebrtcTransportEvent::RemoteIce(ice, res.clone())) {
                        res.answer(200, Err(ServerError::build("REMOTE_ICE_ERROR", err)));
                    }
                    Some(())
                }
                Ok(InternalControl::ForceClose(res)) => {
                    res.send(()).await;
                    None
                }
                Err(e) => {
                    log::error!("Error on endpoint custom recv: {:?}", e);
                    None
                }
            }
        }
    }
}

//TODO avoid error string
pub(crate) async fn run_webrtc_endpoint<C, CE, L>(
    counter: &mut u64,
    conns: Arc<RwLock<HashMap<String, Sender<InternalControl>>>>,
    peers: Arc<RwLock<HashMap<PeerIdentity, Sender<InternalControl>>>>,
    cluster: &mut C,
    life_cycle: L,
    sub_scope: EndpointSubscribeScope,
    bitrate_type: BitrateLimiterType,
    room: &str,
    peer: &str,
    offer_sdp: &str,
    senders: Vec<WebrtcConnectRequestSender>,
    sdp_rewrite: Option<SdpBoxRewriteScope>,
) -> Result<(String, String), ServerError>
where
    C: Cluster<CE> + 'static,
    CE: ClusterEndpoint + 'static,
    L: TransportLifeCycle + 'static,
{
    let (mut session, tx, answer_sdp) = match WebrtcSession::new(room, peer, sub_scope, bitrate_type, life_cycle, cluster, offer_sdp, senders, sdp_rewrite).await {
        Ok(res) => res,
        Err(e) => {
            log::error!("Error on create webrtc session: {:?}", e);
            return Err(ServerError::build("CONNECT_ERROR", format!("{:?}", e)));
        }
    };

    let connect_id = format!("conn-{}", *counter);
    *counter += 1;
    let local_peer_id = PeerIdentity::new(room, peer);
    let peers_c = peers.clone();
    let conns_c = conns.clone();

    conns_c.write().insert(connect_id.clone(), tx.clone());
    if let Some(old_tx) = peers_c.write().insert(local_peer_id.clone(), tx) {
        let (tx, rx) = async_std::channel::bounded(1);
        old_tx.send(InternalControl::ForceClose(tx.clone())).await;
        rx.recv().timeout(Duration::from_secs(1)).await;
    }

    let connect_id_c = connect_id.clone();
    async_std::task::spawn(async move {
        log::info!("[MediaServer] start loop for webrtc endpoint");
        while let Some(_) = session.recv().await {}
        log::info!("[MediaServer] stop loop for webrtc endpoint");
        conns_c.write().remove(&connect_id_c);
        peers_c.write().remove(&local_peer_id);
    });

    Ok((answer_sdp, connect_id))
}
