use async_std::channel::Sender;
use poem::{http::StatusCode, web::Data, Result};
use poem_openapi::{payload::Json, Object, OpenApi};
use serde::{Deserialize, Serialize};
use utils::Response;

use crate::rpc::{RpcEvent, RpcResponse, WebrtcConnectRequest, WebrtcConnectResponse, WhipConnectResponse};

use super::payload_sdp::ApplicationSdp;

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct WebrtcSdp {
    pub node_id: u32,
    pub conn_id: String,
    pub sdp: String,
    /// This is use for provide proof of Price
    pub service_token: Option<String>,
}

pub type WebrtcConnectApiResponse = Response<WebrtcSdp>;

pub struct HttpApis;

#[OpenApi]
impl HttpApis {
    /// connect whip endpoint
    #[oai(path = "/whip/endpoint", method = "post")]
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

    /// connect whip endpoint
    #[oai(path = "/webrtc/connect", method = "post")]
    async fn create_webrtc(&self, ctx: Data<&Sender<RpcEvent>>, body: Json<WebrtcConnectRequest>) -> Result<Json<WebrtcConnectApiResponse>> {
        log::info!("[HttpApis] create webrtc endpoint with sdp {}", body.0.sdp);
        let (res, rx) = RpcResponse::<WebrtcConnectResponse>::new();
        ctx.0
            .send(RpcEvent::WebrtcConnect(body.0, res))
            .await
            .map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whip endpoint created with conn_id {} and sdp {}", res.conn_id, res.sdp);
        Ok(Json(WebrtcConnectApiResponse {
            status: true,
            error: None,
            data: Some(WebrtcSdp {
                node_id: 0,
                conn_id: res.conn_id,
                sdp: res.sdp,
                service_token: None,
            }),
        }))
    }
}
