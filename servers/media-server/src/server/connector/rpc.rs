use ::cluster::rpc::connector::MediaEndpointLogResponse;
use ::cluster::rpc::RpcReqRes;
use protocol::media_event_logs::MediaEndpointLogEvent;

pub mod cluster;
pub mod http;

pub enum RpcEvent {
    MediaEndpointLog(Box<dyn RpcReqRes<MediaEndpointLogEvent, MediaEndpointLogResponse>>),
}
