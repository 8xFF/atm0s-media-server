use poem::{web::Data, Result};
use poem_openapi::{
    auth::Bearer,
    param::Path,
    payload::{Json, Response as HttpResponse},
    OpenApi, SecurityScheme,
};

use super::{
    payload_sdp::{ApplicationSdp, ApplicationSdpPatch, CustomHttpResponse},
    remote_ip::RemoteIpAddr,
    user_agent::UserAgent,
    Response, Rpc,
};

#[derive(SecurityScheme)]
#[oai(rename = "Token Authorization", ty = "bearer", key_in = "header", key_name = "Authorization")]
pub struct TokenAuthorization(pub Bearer);

#[derive(Clone)]
pub struct MediaServerCtx {
    pub(crate) sender: tokio::sync::mpsc::Sender<RpcType>,
}

pub enum RpcType {
    WhipCreate(Rpc<(), ()>),
    WhipPatch(Rpc<(), ()>),
    WhipPost(Rpc<(), ()>),
    WhipDelete(Rpc<(), ()>),
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
        todo!()
    }

    /// patch whip conn for trickle-ice
    #[oai(path = "/whip/conn/:conn_id", method = "patch")]
    async fn conn_whip_patch(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        log::info!("[HttpApis] patch whip endpoint with sdp {}", body.0);
        todo!()
    }

    /// post whip conn for action
    #[oai(path = "/api/whip/conn/:conn_id", method = "post")]
    async fn conn_whip_post(&self, ctx: Data<&MediaServerCtx>, conn_id: Path<String>, body: Json<String>) -> Result<ApplicationSdp<String>> {
        todo!()
    }

    /// delete whip conn
    #[oai(path = "/whip/conn/:conn_id", method = "delete")]
    async fn conn_whip_delete(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>) -> Result<Json<Response<String>>> {
        log::info!("[HttpApis] close whip endpoint conn {}", conn_id.0);
        todo!()
    }
}
