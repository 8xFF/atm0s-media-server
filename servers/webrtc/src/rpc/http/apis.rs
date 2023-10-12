use async_std::channel::Sender;
use poem::{http::StatusCode, web::Data, Result};
use poem_openapi::OpenApi;

use crate::rpc::{RpcEvent, RpcResponse, WhipConnectResponse};

use super::payload_sdp::ApplicationSdp;

pub struct HttpApis;

#[OpenApi]
impl HttpApis {
    /// connect whip endpoint
    #[oai(path = "/api/whip/endpoint", method = "post")]
    async fn create_whip(&self, ctx: Data<&Sender<RpcEvent>>, body: ApplicationSdp<String>) -> Result<ApplicationSdp<String>> {
        log::info!("[HttpApis] create whip endpoint with sdp {}", body.0);
        let (res, rx) = RpcResponse::<WhipConnectResponse>::new();
        ctx.0
            .send(RpcEvent::WhipConnect("token".to_string(), body.0, res))
            .await
            .map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whip endpoint created with location {} and sdp {}", res.location, res.sdp);
        Ok(ApplicationSdp(res.sdp))
    }
}
