use cluster::{ClusterEndpoint, EndpointSubscribeScope};
use media_utils::ServerError;
use transport::Transport;

use crate::{
    endpoint_wrap::BitrateLimiterType,
    rpc::{EndpointRpcIn, EndpointRpcOut, LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    MediaEndpoint,
};

pub struct MediaEndpointPreconditional {
    room: String,
    peer: String,
    subscribe_scope: EndpointSubscribeScope,
    bitrate_type: BitrateLimiterType,
}

impl MediaEndpointPreconditional {
    pub fn new(room: &str, peer: &str, subscribe_scope: EndpointSubscribeScope, bitrate_type: BitrateLimiterType) -> Self {
        Self {
            room: room.into(),
            peer: peer.into(),
            subscribe_scope,
            bitrate_type,
        }
    }

    pub fn check(&mut self) -> Result<(), ServerError> {
        Ok(())
    }

    pub fn build<E, T: Transport<E, EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn, EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>, C: ClusterEndpoint>(
        self,
        transport: T,
        cluster: C,
    ) -> MediaEndpoint<T, E, C> {
        MediaEndpoint::new(transport, cluster, &self.room, &self.peer, self.subscribe_scope, self.bitrate_type)
    }
}
