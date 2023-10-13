use cluster::ClusterEndpoint;
use transport::MediaTransport;
use utils::ServerError;

use crate::{
    rpc::{EndpointRpcIn, EndpointRpcOut},
    MediaEndpoint,
};

pub struct MediaEndpointPreconditional {
    room: String,
    peer: String,
}

impl MediaEndpointPreconditional {
    pub fn new(room: &str, peer: &str) -> Self {
        Self { room: room.into(), peer: peer.into() }
    }

    pub fn check(&mut self) -> Result<(), ServerError> {
        Ok(())
    }

    pub fn build<E, T: MediaTransport<E, EndpointRpcIn, EndpointRpcOut>, C: ClusterEndpoint>(&mut self, transport: T, cluster: C) -> MediaEndpoint<T, E, C> {
        MediaEndpoint::new(transport, cluster, &self.room, &self.peer)
    }
}
