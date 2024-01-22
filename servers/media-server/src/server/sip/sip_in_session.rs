use async_std::channel::Receiver;
use cluster::rpc::general::MediaSessionProtocol;
use cluster::{BitrateControlMode, Cluster, ClusterEndpoint, ClusterEndpointPublishScope, ClusterEndpointSubscribeScope, MixMinusAudioMode};
use endpoint::{MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use futures::{select, FutureExt};
use media_utils::ErrorDebugger;
use transport_sip::{SipTransportIn, LOCAL_TRACK_AUDIO_MAIN};

use super::middleware::sip_incall::SipIncallMiddleware;
use super::InternalControl;

#[derive(Debug)]
pub enum SipInSessionError {
    PreconditionError,
}

pub struct SipInSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<SipTransportIn, (), E>,
    rx: Receiver<InternalControl>,
}

impl<E: ClusterEndpoint> SipInSession<E> {
    pub async fn new<C: Cluster<E>>(room: &str, peer: &str, cluster: &mut C, transport: SipTransportIn, rx: Receiver<InternalControl>) -> Result<Self, SipInSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(
            room,
            peer,
            MediaSessionProtocol::Sip,
            ClusterEndpointPublishScope::Full,
            ClusterEndpointSubscribeScope::Full,
            BitrateControlMode::DynamicWithConsumers,
            MixMinusAudioMode::AllAudioStreams,
            vec![Some(LOCAL_TRACK_AUDIO_MAIN)],
            vec![Box::new(SipIncallMiddleware::new(peer))],
        );
        endpoint_pre.check().map_err(|_e| SipInSessionError::PreconditionError)?;
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
