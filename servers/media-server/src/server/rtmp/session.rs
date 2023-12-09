use std::time::Duration;

use async_std::{channel::Receiver, prelude::FutureExt as _};
use cluster::{Cluster, ClusterEndpoint, EndpointSubscribeScope};
use endpoint::{BitrateLimiterType, MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use futures::{select, FutureExt};
use media_utils::ErrorDebugger;
use transport_rtmp::RtmpTransport;

use crate::server::MediaServerContext;

use super::InternalControl;

#[derive(Debug)]
pub(super) enum RtmpSessionError {
    PreconditionError,
    NetworkError,
}

pub(super) struct RtmpSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<RtmpTransport, (), E>,
    rx: Receiver<InternalControl>,
}

impl<E: ClusterEndpoint> RtmpSession<E> {
    pub async fn new<C: Cluster<E>>(room: &str, peer: &str, cluster: &mut C, transport: RtmpTransport, rx: Receiver<InternalControl>) -> Result<Self, RtmpSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(room, peer, EndpointSubscribeScope::RoomManual, BitrateLimiterType::MaxBitrateOnly);
        endpoint_pre.check().map_err(|_e| RtmpSessionError::PreconditionError)?;
        let room = cluster.build(room, peer);
        let endpoint = endpoint_pre.build(transport, room);

        Ok(Self { endpoint, rx })
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
                Ok(InternalControl::ForceClose(tx)) => {
                    self.endpoint.close().await;
                    tx.send(()).await.log_error("Should send");
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
pub(super) async fn run_rtmp_endpoint<C, CE>(context: MediaServerContext<InternalControl>, cluster: &mut C, room: &str, peer: &str, conn: RtmpTransport) -> Result<String, RtmpSessionError>
where
    C: Cluster<CE> + 'static,
    CE: ClusterEndpoint + 'static,
{
    let (rx, conn_id, old_tx) = context.create_peer(room, peer);
    log::info!("[MediaServer] on rtmp connection from {} {}", room, peer);

    let mut session = RtmpSession::new(&peer, &peer, cluster, conn, rx).await?;

    if let Some(old_tx) = old_tx {
        let (tx, rx) = async_std::channel::bounded(1);
        old_tx.send(InternalControl::ForceClose(tx)).await.log_error("Should send");
        rx.recv().timeout(Duration::from_secs(1)).await.log_error("Should ok");
    }

    let conn_id_c = conn_id.clone();
    async_std::task::spawn(async move {
        log::info!("[MediaServer] start loop for rtmp endpoint");
        while let Some(_) = session.recv().await {}
        log::info!("[MediaServer] stop loop for rtmp endpoint");
        context.close_conn(&conn_id_c);
    });

    Ok(conn_id)
}
