use ::cluster::rpc::connector::{MediaEndpointLogRequest, MediaEndpointLogResponse};
use ::cluster::rpc::RpcReqRes;

pub mod cluster;
pub mod http;

pub enum RpcEvent {
    MediaEndpointLog(Box<dyn RpcReqRes<MediaEndpointLogRequest, MediaEndpointLogResponse>>),
}
