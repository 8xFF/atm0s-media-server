use crate::rpc::http::RpcReqResHttp;
use crate::server::sip::InternalControl;
use crate::server::MediaServerContext;
use async_std::channel::Sender;
use cluster::rpc::sip::SipOutgoingInviteClientRequest;
use cluster::rpc::sip::SipOutgoingInviteResponse;
use cluster::rpc::sip::SipOutgoingInviteServerRequest;
use media_utils::Response;
use poem::http::StatusCode;
use poem::{web::Data, Result};
use poem_openapi::{payload::Json, OpenApi};

use super::RpcEvent;

pub struct SipHttpApis;

#[OpenApi]
impl SipHttpApis {
    /// get node health
    #[oai(path = "/health", method = "get")]
    async fn health(&self, Data(_ctx): Data<&(Sender<RpcEvent>, MediaServerContext<()>)>) -> Result<Json<Response<String, String>>> {
        Ok(Json(Response::success("OK")))
    }

    /// invite a client
    #[oai(path = "/sip/invite/client", method = "post")]
    async fn invite_session(
        &self,
        Data(ctx): Data<&(Sender<RpcEvent>, MediaServerContext<InternalControl>)>,
        body: Json<SipOutgoingInviteClientRequest>,
    ) -> Result<Json<Response<SipOutgoingInviteResponse, String>>> {
        log::info!("[HttpApis] invite sip client {:?}", body.0);
        let (req, rx) = RpcReqResHttp::new(body.0);
        ctx.0
            .send(RpcEvent::InviteOutgoingClient(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        Ok(Json(Response::success(res)))
    }

    /// invite a server
    #[oai(path = "/sip/invite/server", method = "post")]
    async fn invite_server(
        &self,
        Data(ctx): Data<&(Sender<RpcEvent>, MediaServerContext<()>)>,
        body: Json<SipOutgoingInviteServerRequest>,
    ) -> Result<Json<Response<SipOutgoingInviteResponse, String>>> {
        log::info!("[HttpApis] invite sip client {:?}", body.0);
        let (req, rx) = RpcReqResHttp::new(body.0);
        ctx.0
            .send(RpcEvent::InviteOutgoingServer(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        Ok(Json(Response::success(res)))
    }
}
