use crate::server::MediaServerContext;
use async_std::channel::Sender;
use cluster::rpc::sip::SipOutgoingInviteClientRequest;
use cluster::rpc::sip::SipOutgoingInviteResponse;
use media_utils::Response;
use poem::{
    web::{Data, Path},
    Result,
};
use poem_openapi::{payload::Json, OpenApi};

use super::RpcEvent;

pub struct SipHttpApis;

#[OpenApi]
impl SipHttpApis {
    /// invite a session
    #[oai(path = "/sip/invite/session/:session_id", method = "post")]
    async fn invite_session(
        &self,
        Data(ctx): Data<&(Sender<RpcEvent>, MediaServerContext<()>)>,
        body: Json<SipOutgoingInviteClientRequest>,
        session_id: Path<String>,
    ) -> Result<Json<Response<SipOutgoingInviteResponse>>> {
        log::info!("[HttpApis] invite sip session {}", session_id.0);
        Ok(Json(Response {
            status: false,
            error: Some("NOT_IMPLEMENTED".to_string()),
            data: None,
        }))
    }

    /// invite a server
    #[oai(path = "/sip/invite/server", method = "post")]
    async fn invite_server(
        &self,
        Data(ctx): Data<&(Sender<RpcEvent>, MediaServerContext<()>)>,
        body: Json<SipOutgoingInviteClientRequest>,
        session_id: Path<String>,
    ) -> Result<Json<Response<SipOutgoingInviteResponse>>> {
        log::info!("[HttpApis] invite sip session {}", session_id.0);
        Ok(Json(Response {
            status: false,
            error: Some("NOT_IMPLEMENTED".to_string()),
            data: None,
        }))
    }
}
