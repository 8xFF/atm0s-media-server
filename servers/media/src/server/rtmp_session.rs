use cluster::{Cluster, ClusterEndpoint};
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    BitrateLimiterType, EndpointRpcIn, EndpointRpcOut, MediaEndpoint, MediaEndpointOutput, MediaEndpointPreconditional,
};
use media_utils::EndpointSubscribeScope;
use transport_rtmp::RtmpTransport;

type RmIn = EndpointRpcIn;
type RrIn = RemoteTrackRpcIn;
type RlIn = LocalTrackRpcIn;
type RmOut = EndpointRpcOut;
type RrOut = RemoteTrackRpcOut;
type RlOut = LocalTrackRpcOut;

#[derive(Debug)]
pub enum WebrtcSessionError {
    PreconditionError,
    NetworkError,
}

pub struct RtmpSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<RtmpTransport<RmIn, RrIn, RlIn>, (), E>,
}

impl<E: ClusterEndpoint> RtmpSession<E> {
    pub async fn new<C: Cluster<E>>(room: &str, peer: &str, cluster: &mut C, transport: RtmpTransport<RmIn, RrIn, RlIn>) -> Result<Self, WebrtcSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(room, peer, EndpointSubscribeScope::RoomManual, BitrateLimiterType::MaxBitrateOnly);
        endpoint_pre.check().map_err(|_e| WebrtcSessionError::PreconditionError)?;
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
