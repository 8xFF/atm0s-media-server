use cluster::{Cluster, ClusterEndpoint};
use endpoint::{MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use media_utils::EndpointSubscribeScope;
use transport_sip::SipTransport;

#[derive(Debug)]
pub enum SipSessionError {
    PreconditionError,
}

pub struct SipSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<SipTransport, (), E>,
}

impl<E: ClusterEndpoint> SipSession<E> {
    pub async fn new<C: Cluster<E>>(room: &str, peer: &str, cluster: &mut C, transport: SipTransport) -> Result<Self, SipSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(room, peer, EndpointSubscribeScope::RoomManual);
        endpoint_pre.check().map_err(|_e| SipSessionError::PreconditionError)?;
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
