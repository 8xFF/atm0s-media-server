use cluster::ClusterRoom;
use transport::MediaTransport;
use utils::ServerError;

use crate::MediaEndpoint;

pub struct MediaEndpointPreconditional {}

impl MediaEndpointPreconditional {
    pub fn new() -> Self {
        Self {}
    }

    pub fn check(&mut self) -> Result<(), ServerError> {
        Ok(())
    }

    pub fn build<E, T: MediaTransport<E>, C: ClusterRoom>(&mut self, transport: T, cluster: C) -> MediaEndpoint<T, E, C> {
        MediaEndpoint::new(transport, cluster)
    }
}
