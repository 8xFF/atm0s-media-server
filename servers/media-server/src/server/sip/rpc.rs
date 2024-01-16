use ::cluster::rpc::{
    gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
    general::{MediaEndpointCloseRequest, MediaEndpointCloseResponse},
    sip::{SipOutgoingInviteClientRequest, SipOutgoingInviteResponse, SipOutgoingInviteServerRequest},
    RpcReqRes,
};

pub(super) mod cluster;
pub(super) mod http;

pub enum RpcEvent {
    InviteOutgoingClient(Box<dyn RpcReqRes<SipOutgoingInviteClientRequest, SipOutgoingInviteResponse>>),
    InviteOutgoingServer(Box<dyn RpcReqRes<SipOutgoingInviteServerRequest, SipOutgoingInviteResponse>>),
    MediaEndpointClose(Box<dyn RpcReqRes<MediaEndpointCloseRequest, MediaEndpointCloseResponse>>),
}
