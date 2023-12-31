use cluster::{Cluster, ClusterEndpoint};
use cluster::{EndpointSubscribeScope, MixMinusAudioMode};
use endpoint::{BitrateLimiterType, MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional};
use transport_sip::SipTransportOut;

#[derive(Debug)]
pub enum SipOutSessionError {
    PreconditionError,
}

pub struct SipOutSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<SipTransportOut, (), E>,
}

impl<E: ClusterEndpoint> SipOutSession<E> {
    pub async fn new<C: Cluster<E>>(room: &str, peer: &str, cluster: &mut C, transport: SipTransportOut) -> Result<Self, SipOutSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(
            room,
            peer,
            EndpointSubscribeScope::RoomManual,
            BitrateLimiterType::DynamicWithConsumers,
            MixMinusAudioMode::AllAudioStreams,
            1,
        );
        endpoint_pre.check().map_err(|_e| SipOutSessionError::PreconditionError)?;
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
