use std::time::Duration;

use cluster::{Cluster, ClusterEndpoint};
use endpoint::{MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use media_utils::EndpointSubscribeScope;
use transport_sip::SipTransportIn;

#[derive(Debug)]
pub enum SipInSessionError {
    PreconditionError,
}

pub struct SipInSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<SipTransportIn, (), E>,
}

impl<E: ClusterEndpoint> SipInSession<E> {
    pub async fn new<C: Cluster<E>>(room: &str, peer: &str, cluster: &mut C, transport: SipTransportIn) -> Result<Self, SipInSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(room, peer, EndpointSubscribeScope::RoomManual);
        endpoint_pre.check().map_err(|_e| SipInSessionError::PreconditionError)?;
        let room = cluster.build(room, peer);
        let endpoint = endpoint_pre.build(transport, room);

        Ok(Self { endpoint })
    }

    pub async fn recv(&mut self) -> Option<()> {
        match self.endpoint.recv().await {
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
            }
            Err(e) => {
                log::error!("Error on endpoint recv: {:?}", e);
                None
            }
        }
    }
}
