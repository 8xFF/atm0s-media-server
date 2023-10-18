use std::sync::Arc;

use async_std::stream::StreamExt;
use cluster::ClusterEndpoint;
use futures::{select, FutureExt};
use transport::{Transport, TransportError};
use utils::Timer;

use crate::{
    endpoint_wrap::internal::{MediaEndpointInteralEvent, MediaInternalAction},
    rpc::{EndpointRpcIn, EndpointRpcOut, LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
};

use self::internal::MediaEndpointInteral;

mod internal;

pub enum MediaEndpointOutput {
    Continue,
    ConnectionClosed,
}

pub struct MediaEndpoint<T, E, C>
where
    C: ClusterEndpoint,
{
    _tmp_e: std::marker::PhantomData<E>,
    internal: MediaEndpointInteral,
    transport: T,
    cluster: C,
    tick: async_std::stream::Interval,
    timer: Arc<dyn Timer>,
}

impl<T, E, C> MediaEndpoint<T, E, C>
where
    T: Transport<E, EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn, EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>,
    C: ClusterEndpoint,
{
    pub fn new(transport: T, mut cluster: C, room: &str, peer: &str) -> Self {
        log::info!("[EndpointWrap] create");
        //TODO handle error of cluster sub room
        cluster.on_event(cluster::ClusterEndpointOutgoingEvent::SubscribeRoom);
        Self {
            _tmp_e: std::marker::PhantomData,
            internal: MediaEndpointInteral::new(room, peer),
            transport,
            cluster,
            tick: async_std::stream::interval(std::time::Duration::from_millis(100)),
            timer: Arc::new(utils::SystemTimer()),
        }
    }

    pub fn on_custom_event(&mut self, event: E) -> Result<(), TransportError> {
        self.transport.on_custom_event(self.timer.now_ms(), event)
    }

    pub async fn recv(&mut self) -> Result<MediaEndpointOutput, TransportError> {
        while let Some(out) = self.internal.pop_action() {
            match out {
                MediaInternalAction::Internal(e) => match e {
                    MediaEndpointInteralEvent::ConnectionClosed => {
                        return Ok(MediaEndpointOutput::ConnectionClosed);
                    }
                },
                MediaInternalAction::Endpoint(e) => {
                    if let Err(e) = self.transport.on_event(self.timer.now_ms(), e) {
                        return Err(e);
                    }
                }
                MediaInternalAction::Cluster(e) => {
                    if let Err(e) = self.cluster.on_event(e) {
                        todo!("handle error")
                    }
                }
            }
        }

        select! {
            event = self.transport.recv(self.timer.now_ms()).fuse() => {
                self.internal.on_transport(event?);
            },
            event = self.cluster.recv().fuse() => {
                if let Ok(event) = event {
                    self.internal.on_cluster(event);
                }
            }
            _ = self.tick.next().fuse() => {
                self.transport.on_tick(self.timer.now_ms())?;
                self.internal.on_tick(self.timer.now_ms());
            }
        }

        return Ok(MediaEndpointOutput::Continue);
    }
}

impl<T, E, C> Drop for MediaEndpoint<T, E, C>
where
    C: ClusterEndpoint,
{
    fn drop(&mut self) {
        log::info!("[EndpointWrap] drop");
        //TODO handle error of cluster unsub room
        self.cluster.on_event(cluster::ClusterEndpointOutgoingEvent::UnsubscribeRoom);
        self.internal.close();
        while let Some(out) = self.internal.pop_action() {
            match out {
                MediaInternalAction::Cluster(e) => {
                    if let Err(_e) = self.cluster.on_event(e) {
                        todo!("handle error")
                    }
                }
                _ => {}
            }
        }
    }
}
