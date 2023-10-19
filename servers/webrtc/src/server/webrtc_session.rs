use async_std::channel::{bounded, Receiver, Sender};
use cluster::{Cluster, ClusterEndpoint};
use endpoint::{MediaEndpoint, MediaEndpointPreconditional};
use futures::{select, FutureExt};
use utils::ServerError;

use crate::{
    rpc::WebrtcConnectRequestSender,
    transport::{internal::life_cycle::sdk::SdkTransportLifeCycle, WebrtcTransport, WebrtcTransportEvent},
};

use super::InternalControl;

#[derive(Debug)]
pub enum WebrtcSessionError {
    PreconditionError,
    NetworkError,
    SdpError,
}

pub struct WebrtcSession<E: ClusterEndpoint> {
    endpoint: MediaEndpoint<WebrtcTransport<SdkTransportLifeCycle>, WebrtcTransportEvent, E>,
    rx: Receiver<InternalControl>,
}

impl<E: ClusterEndpoint> WebrtcSession<E> {
    pub async fn new<C: Cluster<E>>(
        room: &str,
        peer: &str,
        cluster: &mut C,
        sdp: &str,
        senders: Vec<WebrtcConnectRequestSender>,
        now_ms: u64,
    ) -> Result<(Self, Sender<InternalControl>, String), WebrtcSessionError> {
        let mut endpoint_pre = MediaEndpointPreconditional::new(room, peer);
        endpoint_pre.check().map_err(|_e| WebrtcSessionError::PreconditionError)?;
        let room = cluster.build(room, peer);
        let mut transport = WebrtcTransport::new(SdkTransportLifeCycle::new(now_ms)).await.map_err(|_| WebrtcSessionError::NetworkError)?;
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
                Ok(_) => Some(()),
                Err(e) => {
                    log::error!("Error on endpoint recv: {:?}", e);
                    None
                }
            },
            e = self.rx.recv().fuse() => match e {
                Ok(InternalControl::RemoteIce(ice, mut res)) => {
                    if let Err(err) = self.endpoint.on_custom_event(WebrtcTransportEvent::RemoteIce(ice)) {
                        res.answer(200, Err(ServerError::build("REMOTE_ICE_ERROR", err)));
                    } else {
                        res.answer(200, Ok(()));
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
