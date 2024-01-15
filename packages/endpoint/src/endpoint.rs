use std::sync::Arc;

use async_std::stream::StreamExt;
use cluster::{rpc::general::MediaSessionProtocol, BitrateControlMode, ClusterEndpoint, ClusterEndpointPublishScope, ClusterEndpointSubscribeScope, MixMinusAudioMode};
use futures::{select, FutureExt};
use media_utils::Timer;
use transport::{Transport, TransportError};

use crate::{
    endpoint::internal::{MediaEndpointInternalEvent, MediaInternalAction},
    rpc::{EndpointRpcIn, EndpointRpcOut, LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
};

use self::{internal::MediaEndpointInternal, middleware::MediaEndpointMiddleware};

pub mod internal;
pub mod middleware;

const DEFAULT_MIX_MINUS_NAME: &str = "default";
const DEFAULT_MIX_MINUS_VIRTUAL_TRACK_ID: u16 = 200;

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
}

impl<T, E, C> MediaEndpoint<T, E, C>
where
    T: Transport<E, EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn, EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>,
    C: ClusterEndpoint,
{
    pub fn new(
        transport: T,
        cluster: C,
        room: &str,
        peer: &str,
        protocol: MediaSessionProtocol,
        sub_scope: ClusterEndpointSubscribeScope,
        pub_scope: ClusterEndpointPublishScope,
        bitrate_mode: BitrateControlMode,
        mix_minus_mode: MixMinusAudioMode,
        mix_minus_slots: &[Option<u16>],
        mut middlewares: Vec<Box<dyn MediaEndpointMiddleware>>,
    ) -> Self {
        log::info!("[EndpointWrap] create");
        let timer = Arc::new(media_utils::SystemTimer());
        middlewares.push(Box::new(middleware::logger::MediaEndpointEventLogger::new()));
        if mix_minus_mode != MixMinusAudioMode::Disabled {
            middlewares.push(Box::new(middleware::mix_minus::MixMinusEndpointMiddleware::new(
                room,
                peer,
                DEFAULT_MIX_MINUS_NAME,
                mix_minus_mode,
                DEFAULT_MIX_MINUS_VIRTUAL_TRACK_ID,
                mix_minus_slots,
            )));
        }
        let mut internal = MediaEndpointInternal::new(room, peer, protocol, sub_scope, pub_scope, bitrate_mode, middlewares);
        internal.on_start(timer.now_ms());

        Self {
            _tmp_e: std::marker::PhantomData,
            internal,
            transport,
            cluster,
            tick: async_std::stream::interval(std::time::Duration::from_millis(100)),
            timer,
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
                    MediaEndpointInternalEvent::ConnectionError(e) => {
                        return Err(e);
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
                    Err(e) => self.internal.on_transport_error(self.timer.now_ms(), e),
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
        log::info!("[EndpointWrap] close request");
        self.transport.close().await;
    }
}

impl<T, E, C> Drop for MediaEndpoint<T, E, C>
where
    C: ClusterEndpoint,
{
    fn drop(&mut self) {
        log::info!("[EndpointWrap] drop");
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
