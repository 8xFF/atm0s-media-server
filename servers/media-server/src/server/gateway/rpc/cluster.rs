use std::marker::PhantomData;

use cluster::rpc::{RpcEmitter, RpcEndpoint, RpcRequest, RPC_NODE_PING};

use super::RpcEvent;

pub struct GatewayClusterRpc<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> {
    _tmp: PhantomData<(Req, Emitter)>,
    rpc: RPC,
}

impl<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> GatewayClusterRpc<RPC, Req, Emitter> {
    pub fn new(rpc: RPC) -> Self {
        Self { _tmp: Default::default(), rpc }
    }

    pub fn emitter(&mut self) -> Emitter {
        self.rpc.emitter()
    }

    pub async fn recv(&mut self) -> Option<RpcEvent> {
        loop {
            let event = self.rpc.recv().await?;
            match event.cmd() {
                RPC_NODE_PING => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::NodePing(req));
                    }
                }
                _ => {
                    event.error("NOT_SUPPORTED_CMD");
                }
            }
        }
    }
}
