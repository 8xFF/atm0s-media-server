use media_server_protocol::transport::{
    whip::{self, WhipConnectReq, WhipDeleteReq, WhipRemoteIceReq},
    RpcReq, RpcRes, RpcResult,
};
use poem::{http::StatusCode, web::Data, Result};
use poem_openapi::{
    auth::Bearer,
    param::Path,
    payload::{Json, PlainText, Response as HttpResponse},
    OpenApi, SecurityScheme,
};

use super::{
    utils::{ApplicationSdp, ApplicationSdpPatch, CustomHttpResponse, RemoteIpAddr, UserAgent},
    Rpc,
};

#[derive(SecurityScheme)]
#[oai(rename = "Token Authorization", ty = "bearer", key_in = "header", key_name = "Authorization")]
pub struct TokenAuthorization(pub Bearer);

#[derive(Clone)]
pub struct MediaServerCtx {
    pub(crate) sender: tokio::sync::mpsc::Sender<Rpc<RpcReq, RpcRes>>,
}

pub struct MediaApis;

#[OpenApi]
impl MediaApis {
    /// connect whip endpoint
    #[oai(path = "/whip/endpoint", method = "post")]
    async fn whip_create(
        &self,
        Data(data): Data<&MediaServerCtx>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        log::info!("[MediaAPIs] create whip endpoint with token {}, ip {}, user_agent {}", token.token, ip_addr, user_agent);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Connect(WhipConnectReq {
            ip: ip_addr,
            sdp: body.0,
            token: token.token,
            user_agent,
        })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Whip(whip::RpcRes::Connect(res)) => match res {
                RpcResult::Success(res) => {
                    log::info!("[HttpApis] Whip endpoint created with conn_id {}", res.conn_id);
                    Ok(CustomHttpResponse {
                        code: StatusCode::CREATED,
                        res: ApplicationSdp(res.sdp),
                        headers: vec![("location", format!("/whip/conn/{}", res.conn_id))],
                    })
                }
                RpcResult::Error { code, message } => {
                    log::warn!("Whip endpoint creation failed with code {} and message {}", code, message);
                    Err(poem::Error::from_string(format!("Code: {code}, Message: {message}"), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// patch whip conn for trickle-ice
    #[oai(path = "/whip/conn/:conn_id", method = "patch")]
    async fn conn_whip_patch(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        log::info!("[HttpApis] patch whip endpoint with sdp {}", body.0);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::RemoteIce(WhipRemoteIceReq {
            conn_id: conn_id.0.clone(),
            ice: body.0,
        })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whip(whip::RpcRes::RemoteIce(res)) => match res {
                RpcResult::Success(_res) => {
                    log::info!("[HttpApis] Whip endpoint patch trickle-ice with conn_id {}", conn_id.0);
                    Ok(HttpResponse::new(ApplicationSdpPatch("".to_string())).status(StatusCode::NO_CONTENT))
                }
                RpcResult::Error { code, message } => {
                    log::warn!("Whip endpoint patch trickle-ice failed with code {} and message {}", code, message);
                    Err(poem::Error::from_string(format!("Code: {code}, Message: {message}"), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// post whip conn for action
    #[oai(path = "/api/whip/conn/:conn_id", method = "post")]
    async fn conn_whip_post(&self, _ctx: Data<&MediaServerCtx>, _conn_id: Path<String>, _body: Json<String>) -> Result<ApplicationSdp<String>> {
        Err(poem::Error::from_string("Not supported", StatusCode::BAD_REQUEST))
    }

    /// delete whip conn
    #[oai(path = "/whip/conn/:conn_id", method = "delete")]
    async fn conn_whip_delete(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>) -> Result<PlainText<String>> {
        log::info!("[HttpApis] close whip endpoint conn {}", conn_id.0);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Delete(WhipDeleteReq { conn_id: conn_id.0.clone() })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Whip(whip::RpcRes::Delete(res)) => match res {
                RpcResult::Success(_res) => {
                    log::info!("[HttpApis] Whip endpoint closed with conn_id {}", conn_id.0);
                    Ok(PlainText("OK".to_string()))
                }
                RpcResult::Error { code, message } => {
                    log::warn!("Whip endpoint close request failed with code {} and message {}", code, message);
                    Err(poem::Error::from_string(format!("Code: {code}, Message: {message}"), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }
}
