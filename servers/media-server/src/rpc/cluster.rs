// use std::marker::PhantomData;

// use cluster::{RpcEndpoint, RpcRequest, RPC_WEBRTC_CONNECT, RPC_WEBRTC_ICE};

// pub struct ClusterRpc<RPC: RpcEndpoint<Req>, Req: RpcRequest> {
//     _tmp: PhantomData<Req>,
//     rpc: RPC
// }

// impl<RPC: RpcEndpoint<Req>, Req: RpcRequest> ClusterRpc<RPC, Req> {
//     pub fn new(rpc: RPC) -> Self {
//         Self {
//             _tmp: Default::default(),
//             rpc
//         }
//     }

//     pub async fn recv(&mut self) -> Option<RpcEvent> {
//         loop {
//             let event = self.rpc.recv().await?;
//             match event.cmd() {
//                 RPC_WEBRTC_CONNECT => {
//                     return RpcEvent::WebrtcConnect(event.parse());
//                 }
//                 RPC_WEBRTC_ICE => {
//                     return RpcEvent::WebrtcRemoteIce(event.parse());
//                 },
//                 _ => {
//                     event.answer(Err("NOT_SUPPORTED_CMD"));
//                 }
//             }
//         }
//     }
// }

