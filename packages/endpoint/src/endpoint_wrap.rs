use std::{collections::HashMap, sync::Arc};

use async_std::stream::StreamExt;
use cluster::ClusterEndpoint;
use futures::{select, FutureExt};
use media_utils::{EndpointSubscribeScope, Timer};
use transport::{Transport, TransportError};

use crate::{
    endpoint_wrap::internal::{MediaEndpointInternalEvent, MediaInternalAction},
    rpc::{EndpointRpcIn, EndpointRpcOut, LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
};

use self::internal::MediaEndpointInternal;

mod internal;
pub use internal::BitrateLimiterType;

pub enum MediaEndpointOutput {
    Continue,
    ConnectionClosed,
    ConnectionCloseRequest,
}

pub struct MediaEndpoint<T, E, C>
where
    C: ClusterEndpoint,
{
    _tmp_e: std::marker::PhantomData<E>,
    internal: MediaEndpointInternal,
    transport: T,
    cluster: C,
    tick: async_std::stream::Interval,
    timer: Arc<dyn Timer>,
    sub_scope: EndpointSubscribeScope,
    peer_subscribe: HashMap<String, ()>,
}

impl<T, E, C> MediaEndpoint<T, E, C>
where
    T: Transport<E, EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn, EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>,
    C: ClusterEndpoint,
{
    pub fn new(transport: T, mut cluster: C, room: &str, peer: &str, sub_scope: EndpointSubscribeScope, bitrate_type: BitrateLimiterType) -> Self {
        log::info!("[EndpointWrap] create");
        //TODO handle error of cluster sub room
        if matches!(sub_scope, EndpointSubscribeScope::RoomAuto) {
            if let Err(_e) = cluster.on_event(cluster::ClusterEndpointOutgoingEvent::SubscribeRoom) {
                todo!("handle error")
            }
        }
        Self {
            _tmp_e: std::marker::PhantomData,
            internal: MediaEndpointInternal::new(room, peer, bitrate_type),
            transport,
            cluster,
            tick: async_std::stream::interval(std::time::Duration::from_millis(100)),
            timer: Arc::new(media_utils::SystemTimer()),
            sub_scope,
            peer_subscribe: HashMap::new(),
        }
    }

    pub fn on_custom_event(&mut self, event: E) -> Result<(), TransportError> {
        self.transport.on_custom_event(self.timer.now_ms(), event)
    }

    pub async fn recv(&mut self) -> Result<MediaEndpointOutput, TransportError> {
        while let Some(out) = self.internal.pop_action() {
            match out {
                MediaInternalAction::Internal(e) => match e {
                    MediaEndpointInternalEvent::ConnectionClosed => {
                        return Ok(MediaEndpointOutput::ConnectionClosed);
                    }
                    MediaEndpointInternalEvent::ConnectionCloseRequest => {
                        return Ok(MediaEndpointOutput::ConnectionCloseRequest);
                    }
                    MediaEndpointInternalEvent::SubscribePeer(peer) => {
                        if matches!(self.sub_scope, EndpointSubscribeScope::RoomManual) {
                            self.peer_subscribe.insert(peer.clone(), ());
                            if let Err(_e) = self.cluster.on_event(cluster::ClusterEndpointOutgoingEvent::SubscribePeer(peer)) {
                                todo!("handle error")
                            }
                        }
                    }
                    MediaEndpointInternalEvent::UnsubscribePeer(peer) => {
                        if matches!(self.sub_scope, EndpointSubscribeScope::RoomManual) {
                            self.peer_subscribe.remove(&peer);
                            if let Err(_e) = self.cluster.on_event(cluster::ClusterEndpointOutgoingEvent::UnsubscribePeer(peer)) {
                                todo!("handle error")
                            }
                        }
                    }
                },
                MediaInternalAction::Endpoint(e) => {
                    if let Err(e) = self.transport.on_event(self.timer.now_ms(), e) {
                        //only ending session if is critical error
                        match &e {
                            TransportError::ConnectError(_) => return Err(e),
                            TransportError::ConnectionError(_) => return Err(e),
                            TransportError::NetworkError => {}
                            TransportError::RuntimeError(_) => {}
                        }
                    }
                }
                MediaInternalAction::Cluster(e) => {
                    if let Err(_e) = self.cluster.on_event(e) {
                        todo!("handle error")
                    }
                }
            }
        }

        select! {
            event = self.transport.recv(self.timer.now_ms()).fuse() => {
                match event {
                    Ok(event) => self.internal.on_transport(self.timer.now_ms(), event),
                    //only ending session if is critical error
                    Err(e) => match &e {
                        TransportError::ConnectError(_) => return Err(e),
                        TransportError::ConnectionError(_) => return Err(e),
                        TransportError::NetworkError => {},
                        TransportError::RuntimeError(_) => {},
                    }
                }
            },
            event = self.cluster.recv().fuse() => {
                if let Ok(event) = event {
                    self.internal.on_cluster(self.timer.now_ms(), event);
                }
            }
            _ = self.tick.next().fuse() => {
                self.transport.on_tick(self.timer.now_ms())?;
                self.internal.on_tick(self.timer.now_ms());
            }
        }

        return Ok(MediaEndpointOutput::Continue);
    }

    pub async fn close(&mut self) {
        self.transport.close().await;
    }
}

impl<T, E, C> Drop for MediaEndpoint<T, E, C>
where
    C: ClusterEndpoint,
{
    fn drop(&mut self) {
        log::info!("[EndpointWrap] drop");
        match self.sub_scope {
            EndpointSubscribeScope::RoomAuto => {
                if let Err(_e) = self.cluster.on_event(cluster::ClusterEndpointOutgoingEvent::UnsubscribeRoom) {
                    todo!("handle error")
                }
            }
            EndpointSubscribeScope::RoomManual => {
                for peer in self.peer_subscribe.keys() {
                    if let Err(_e) = self.cluster.on_event(cluster::ClusterEndpointOutgoingEvent::UnsubscribePeer(peer.clone())) {
                        todo!("handle error")
                    }
                }
            }
        }
        self.internal.before_drop(self.timer.now_ms());
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
