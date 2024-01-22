use std::marker::PhantomData;

use cluster::rpc::{
    gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
    RpcEmitter, RpcEndpoint, RpcRequest, RPC_MEDIA_ENDPOINT_CLOSE, RPC_NODE_HEALTHCHECK,
};

use super::RpcEvent;

pub struct RtmpClusterRpc<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> {
    _tmp: PhantomData<(Req, Emitter)>,
    rpc: RPC,
}

impl<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> RtmpClusterRpc<RPC, Req, Emitter> {
    pub fn new(rpc: RPC) -> Self {
        Self { _tmp: Default::default(), rpc }
    }

    pub fn emitter(&mut self) -> Emitter {
        self.rpc.emitter()
    }

    pub async fn recv(&mut self) -> Option<RpcEvent> {
        loop {
            let req = self.rpc.recv().await?;
            match req.cmd() {
                RPC_NODE_HEALTHCHECK => {
                    if let Some(req) = req.parse::<NodeHealthcheckRequest, _>() {
                        req.answer(Ok(NodeHealthcheckResponse { success: true }));
                    }
                }
                RPC_MEDIA_ENDPOINT_CLOSE => {
                    if let Some(req) = req.parse() {
                        return Some(RpcEvent::MediaEndpointClose(req));
                    }
                }
                _ => {
                    req.error("NOT_SUPPORTED_CMD");
                }
            }
        }
    }
}
