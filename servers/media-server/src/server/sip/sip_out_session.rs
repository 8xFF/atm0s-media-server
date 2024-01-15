use async_std::channel::Receiver;
use cluster::rpc::general::MediaSessionProtocol;
use cluster::{BitrateControlMode, Cluster, ClusterEndpoint, ClusterEndpointPublishScope, ClusterEndpointSubscribeScope, MixMinusAudioMode};
use endpoint::{MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use futures::{select, FutureExt};
use media_utils::ErrorDebugger;
use transport_sip::SipTransportOut;

use super::InternalControl;

#[derive(Debug)]
pub enum SipOutSessionError {
    PreconditionError,
}

pub struct SipOutSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<SipTransportOut, (), E>,
    rx: Receiver<InternalControl>,
}

impl<E: ClusterEndpoint> SipOutSession<E> {
    pub async fn new<C: Cluster<E>>(room: &str, peer: &str, cluster: &mut C, transport: SipTransportOut, rx: Receiver<InternalControl>) -> Result<Self, SipOutSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(
            room,
            peer,
            MediaSessionProtocol::Sip,
            ClusterEndpointPublishScope::Full,
            ClusterEndpointSubscribeScope::Full,
            BitrateControlMode::DynamicWithConsumers,
            MixMinusAudioMode::AllAudioStreams,
            1,
        );
        endpoint_pre.check().map_err(|_e| SipOutSessionError::PreconditionError)?;
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
