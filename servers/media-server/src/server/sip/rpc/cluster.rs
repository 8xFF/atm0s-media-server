use std::marker::PhantomData;

use cluster::rpc::{
    gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
    RpcEmitter, RpcEndpoint, RpcRequest, RPC_MEDIA_ENDPOINT_CLOSE, RPC_NODE_HEALTHCHECK, RPC_SIP_INVITE_OUTGOING_CLIENT, RPC_SIP_INVITE_OUTGOING_SERVER,
};

use super::RpcEvent;

pub struct SipClusterRpc<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> {
    _tmp: PhantomData<(Req, Emitter)>,
    rpc: RPC,
}

impl<RPC: RpcEndpoint<Req, Emitter>, Req: RpcRequest, Emitter: RpcEmitter> SipClusterRpc<RPC, Req, Emitter> {
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
                RPC_NODE_HEALTHCHECK => {
                    if let Some(req) = event.parse::<NodeHealthcheckRequest, _>() {
                        req.answer(Ok(NodeHealthcheckResponse { success: true }));
                    }
                }
                RPC_MEDIA_ENDPOINT_CLOSE => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::MediaEndpointClose(req));
                    }
                }
                RPC_SIP_INVITE_OUTGOING_CLIENT => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::InviteOutgoingClient(req));
                    }
                }
                RPC_SIP_INVITE_OUTGOING_SERVER => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::InviteOutgoingServer(req));
                    }
                }
                _ => {
                    event.error("NOT_SUPPORTED_CMD");
                }
            }
        }
    }
}
