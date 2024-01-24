use std::marker::PhantomData;

use cluster::rpc::{
    gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
    RpcEmitter, RpcEndpoint, RpcRequest, RPC_MEDIA_ENDPOINT_CLOSE, RPC_NODE_HEALTHCHECK, RPC_NODE_PING, RPC_WEBRTC_CONNECT, RPC_WEBRTC_ICE, RPC_WEBRTC_PATCH, RPC_WHEP_CONNECT, RPC_WHIP_CONNECT,
};

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
                RPC_NODE_HEALTHCHECK => {
                    if let Some(req) = event.parse::<NodeHealthcheckRequest, _>() {
                        req.answer(Ok(NodeHealthcheckResponse { success: true }));
                    }
                }
                RPC_NODE_PING => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::NodePing(req));
                    }
                }
                RPC_WEBRTC_CONNECT => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::WebrtcConnect(req));
                    }
                }
                RPC_WEBRTC_ICE => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::WebrtcRemoteIce(req));
                    }
                }
                RPC_WEBRTC_PATCH => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::WebrtcSdpPatch(req));
                    }
                }
                RPC_MEDIA_ENDPOINT_CLOSE => {
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::MediaEndpointClose(req));
                    }
                }
                RPC_WHIP_CONNECT => {
                    log::info!("[MediaServer][Webrtc] on whip connect request");
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::WhipConnect(req));
                    }
                }
                RPC_WHEP_CONNECT => {
                    log::info!("[MediaServer][Webrtc] on whep connect request");
                    if let Some(req) = event.parse() {
                        return Some(RpcEvent::WhepConnect(req));
                    }
                }
                _ => {
                    event.error("NOT_SUPPORTED_CMD");
                }
            }
        }
    }
}
