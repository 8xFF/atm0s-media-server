use cluster::{rpc::general::MediaSessionProtocol, BitrateControlMode, ClusterEndpoint, ClusterEndpointPublishScope, ClusterEndpointSubscribeScope, MixMinusAudioMode};
use media_utils::ServerError;
use transport::Transport;

use crate::{
    endpoint::middleware::MediaEndpointMiddleware,
    rpc::{EndpointRpcIn, EndpointRpcOut, LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    MediaEndpoint,
};

pub struct MediaEndpointPreconditional {
    room: String,
    peer: String,
    protocol: MediaSessionProtocol,
    pub_scope: ClusterEndpointPublishScope,
    sub_scope: ClusterEndpointSubscribeScope,
    bitrate_mode: BitrateControlMode,
    mix_minus_mode: MixMinusAudioMode,
    mix_minus_slots: Vec<Option<u16>>,
    middlewares: Vec<Box<dyn MediaEndpointMiddleware>>,
}

impl MediaEndpointPreconditional {
    pub fn new(
        room: &str,
        peer: &str,
        protocol: MediaSessionProtocol,
        pub_scope: ClusterEndpointPublishScope,
        sub_scope: ClusterEndpointSubscribeScope,
        bitrate_mode: BitrateControlMode,
        mix_minus_mode: MixMinusAudioMode,
        mix_minus_slots: Vec<Option<u16>>,
        middlewares: Vec<Box<dyn MediaEndpointMiddleware>>,
    ) -> Self {
        Self {
            room: room.into(),
            peer: peer.into(),
            protocol,
            sub_scope,
            pub_scope,
            bitrate_mode,
            mix_minus_mode,
            mix_minus_slots,
            middlewares,
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
            transport,
            cluster,
            &self.room,
            &self.peer,
            self.protocol,
            self.sub_scope,
            self.pub_scope,
            self.bitrate_mode,
            self.mix_minus_mode,
            &self.mix_minus_slots,
            self.middlewares,
        )
    }
}
