use std::marker::PhantomData;

use cluster::rpc::{RpcEmitter, RpcEndpoint, RpcRequest, RPC_MEDIA_ENDPOINT_LOG};

use super::RpcEvent;

pub struct ConnectorClusterRpc<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> {
    _tmp: PhantomData<(Req, Emitter)>,
    rpc: RPC,
}

impl<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> ConnectorClusterRpc<RPC, Req, Emitter> {
    pub fn new(rpc: RPC) -> Self {
        Self { _tmp: Default::default(), rpc }
    }

    #[allow(unused)]
    pub fn emitter(&mut self) -> Emitter {
        self.rpc.emitter()
    }

    pub async fn recv(&mut self) -> Option<RpcEvent> {
        loop {
            let event = self.rpc.recv().await?;
            match event.cmd() {
                RPC_MEDIA_ENDPOINT_LOG => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::MediaEndpointLog(req));
                    }
                }
                _ => {
                    event.error("NOT_SUPPORTED_CMD");
                }
            }
        }
    }
}
