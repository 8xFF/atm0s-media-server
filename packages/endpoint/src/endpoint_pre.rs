use cluster::{ClusterEndpoint, EndpointSubscribeScope, MixMinusAudioMode};
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
    mix_minus_mode: MixMinusAudioMode,
    mix_minus_size: usize,
}

impl MediaEndpointPreconditional {
    pub fn new(room: &str, peer: &str, subscribe_scope: EndpointSubscribeScope, bitrate_type: BitrateLimiterType, mix_minus_mode: MixMinusAudioMode, mix_minus_size: usize) -> Self {
        Self {
            room: room.into(),
            peer: peer.into(),
            subscribe_scope,
            bitrate_type,
            mix_minus_mode,
            mix_minus_size,
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
        MediaEndpoint::new(
            transport, cluster, &self.room, &self.peer, self.subscribe_scope, self.bitrate_type, self.mix_minus_mode, self.mix_minus_size,
        )
    }
}
