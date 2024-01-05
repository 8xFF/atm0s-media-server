use ::cluster::rpc::connector::MediaEndpointLogResponse;
use ::cluster::rpc::RpcReqRes;
use protocol::media_event_logs::MediaEndpointLogRequest;

pub mod cluster;
pub mod http;

pub enum InternalControl {}
pub enum RpcEvent {
    MediaEndpointLog(Box<dyn RpcReqRes<MediaEndpointLogRequest, MediaEndpointLogResponse>>),
}
