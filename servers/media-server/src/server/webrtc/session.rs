use async_std::{channel::Receiver, prelude::FutureExt as _};
use cluster::{
    rpc::{general::MediaSessionProtocol, webrtc::WebrtcConnectRequestSender},
    BitrateControlMode, Cluster, ClusterEndpoint, ClusterEndpointPublishScope, ClusterEndpointSubscribeScope, MixMinusAudioMode,
};
use endpoint::{MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use futures::{select, FutureExt};
use media_utils::{ErrorDebugger, ServerError};
use std::time::Duration;
use transport_webrtc::{SdpBoxRewriteScope, TransportLifeCycle, WebrtcTransport, WebrtcTransportEvent};

use crate::server::MediaServerContext;

use super::InternalControl;

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
        protocol: MediaSessionProtocol,
        pub_scope: ClusterEndpointPublishScope,
        sub_scope: ClusterEndpointSubscribeScope,
        bitrate_mode: BitrateControlMode,
        life_cycle: L,
        cluster: &mut C,
        sdp: &str,
        senders: Vec<WebrtcConnectRequestSender>,
        sdp_rewrite: Option<SdpBoxRewriteScope>,
        rx: Receiver<InternalControl>,
        mix_minus_mode: MixMinusAudioMode,
        mix_minus_size: usize,
    ) -> Result<(Self, String), WebrtcSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(room, peer, protocol, pub_scope, sub_scope, bitrate_mode, mix_minus_mode, mix_minus_size);
        endpoint_pre.check().map_err(|_e| WebrtcSessionError::PreconditionError)?;
        let room = cluster.build(room, peer);
        let mut transport = WebrtcTransport::new(life_cycle, sdp_rewrite).await.map_err(|_| WebrtcSessionError::NetworkError)?;
        for sender in senders {
            transport.map_remote_stream(sender);
        }
        let answer = transport.on_remote_sdp(sdp).map_err(|_| WebrtcSessionError::SdpError)?;
        let endpoint = endpoint_pre.build(transport, room);

        Ok((Self { endpoint, rx }, answer))
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
                Ok(InternalControl::RemoteIce(req)) => {
                    self.endpoint.on_custom_event(WebrtcTransportEvent::RemoteIce(req)).log_error("Should ok");
                    Some(())
                }
                Ok(InternalControl::SdpPatch(req)) => {
                    self.endpoint.on_custom_event(WebrtcTransportEvent::SdpPatch(req)).log_error("Should ok");
                    Some(())
                }
                Ok(InternalControl::ForceClose(tx)) => {
                    self.endpoint.close().await;
                    tx.send(()).await.log_error("Should send");
                    Some(())
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
    context: MediaServerContext<InternalControl>,
    cluster: &mut C,
    life_cycle: L,
    protocol: MediaSessionProtocol,
    pub_scope: ClusterEndpointPublishScope,
    sub_scope: ClusterEndpointSubscribeScope,
    bitrate_mode: BitrateControlMode,
    room: &str,
    peer: &str,
    offer_sdp: &str,
    senders: Vec<WebrtcConnectRequestSender>,
    sdp_rewrite: Option<SdpBoxRewriteScope>,
    mix_minus_mode: MixMinusAudioMode,
    mix_minus_size: usize,
) -> Result<(String, String), ServerError>
where
    C: Cluster<CE> + 'static,
    CE: ClusterEndpoint + 'static,
    L: TransportLifeCycle + 'static,
{
    let (rx, conn_id, old_tx) = context.create_peer(room, peer, None);
    let (mut session, answer_sdp) = match WebrtcSession::new(
        room, peer, protocol, pub_scope, sub_scope, bitrate_mode, life_cycle, cluster, offer_sdp, senders, sdp_rewrite, rx, mix_minus_mode, mix_minus_size,
    )
    .await
    {
        Ok(res) => res,
        Err(e) => {
            log::error!("Error on create webrtc session: {:?}", e);
            context.close_conn(&conn_id);
            return Err(ServerError::build("CONNECT_ERROR", format!("{:?}", e)));
        }
    };

    if let Some(old_tx) = old_tx {
        let (tx, rx) = async_std::channel::bounded(1);
        old_tx.send(InternalControl::ForceClose(tx)).await.log_error("Should send");
        rx.recv().timeout(Duration::from_secs(1)).await.log_error("Should ok");
    }

    let conn_id_c = conn_id.clone();
    async_std::task::spawn(async move {
        log::info!("[MediaServer] start loop for webrtc endpoint");
        while let Some(_) = session.recv().await {}
        log::info!("[MediaServer] stop loop for webrtc endpoint");
        context.close_conn(&conn_id_c);
    });

    Ok((answer_sdp, conn_id))
}
