use cluster::ClusterRoom;
use futures::{select, FutureExt};
use transport::{MediaTransport, MediaTransportError};

use crate::endpoint::internal::MediaInternalAction;

use self::internal::MediaEndpointInteral;

mod internal;

pub struct MediaEndpoint<T, E, C> {
    _tmp_e: std::marker::PhantomData<E>,
    internal: MediaEndpointInteral,
    transport: T,
    cluster: C,
}

impl<T, E, C> MediaEndpoint<T, E, C>
where
    T: MediaTransport<E>,
    C: ClusterRoom,
{
    pub fn new(transport: T, cluster: C) -> Self {
        Self {
            _tmp_e: std::marker::PhantomData,
            internal: MediaEndpointInteral::new(),
            transport,
            cluster,
        }
    }

    pub fn on_custom_event(&mut self, event: E) -> Result<(), MediaTransportError> {
        self.transport.on_custom_event(event)
    }

    pub async fn recv(&mut self) -> Result<(), MediaTransportError> {
        select! {
            event = self.transport.recv().fuse() => {
                if let Ok(event) = event {
                    self.internal.on_transport(event);
                }
            },
            event = self.cluster.recv().fuse() => {
                if let Ok(event) = event {
                    self.internal.on_cluster(event);
                }
            }
        }

        while let Some(out) = self.internal.pop_action() {
            match out {
                MediaInternalAction::Endpoint(e) => {
                    if let Err(e) = self.transport.on_event(e) {
                        todo!("handle error")
                    }
                },
                MediaInternalAction::Cluster(e) => {
                    if let Err(e) = self.cluster.on_event(e) {
                        todo!("handle error")
                    }
                },
            }
        }

        Ok(())
    }
}
