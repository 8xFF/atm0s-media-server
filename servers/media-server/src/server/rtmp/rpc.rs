use ::cluster::rpc::{
    gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
    general::{MediaEndpointCloseRequest, MediaEndpointCloseResponse},
    RpcReqRes,
};

pub(super) mod cluster;
pub(super) mod http;

pub enum RpcEvent {
    NodeHeathcheck(Box<dyn RpcReqRes<NodeHealthcheckRequest, NodeHealthcheckResponse>>),
    MediaEndpointClose(Box<dyn RpcReqRes<MediaEndpointCloseRequest, MediaEndpointCloseResponse>>),
}
