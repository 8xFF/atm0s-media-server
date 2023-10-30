use async_std::channel::Sender;
use poem::{http::StatusCode, web::Data, Result};
use poem_openapi::{payload::Json, Object, OpenApi};
use serde::{Deserialize, Serialize};
use transport::RpcResponse;
use transport_webrtc::{WebrtcRemoteIceResponse, WhipConnectResponse, WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest};
use utils::Response;

use crate::rpc::RpcEvent;

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
pub type WebrtcIceRemoteApiResponse = Response<WebrtcRemoteIceResponse>;

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

    /// connect webrtc endpoint
    #[oai(path = "/webrtc/connect", method = "post")]
    async fn create_webrtc(&self, ctx: Data<&Sender<RpcEvent>>, body: Json<WebrtcConnectRequest>) -> Result<Json<WebrtcConnectApiResponse>> {
        log::info!("[HttpApis] create Webrtc endpoint {}/{}", body.0.room, body.0.peer);
        let (res, rx) = RpcResponse::<WebrtcConnectResponse>::new();
        ctx.0
            .send(RpcEvent::WebrtcConnect(body.0, res))
            .await
            .map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Webrtc endpoint created with conn_id {}", res.conn_id);
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

    /// sending remote ice candidate
    #[oai(path = "/webrtc/ice_remote", method = "post")]
    async fn webrtc_ice_remote(&self, ctx: Data<&Sender<RpcEvent>>, body: Json<WebrtcRemoteIceRequest>) -> Result<Json<WebrtcIceRemoteApiResponse>> {
        log::info!("[HttpApis] on Webrtc endpoint ice-remote {}/{:?}", body.0.candidate, body.0.sdp_mid);
        let (res, rx) = RpcResponse::<()>::new();
        ctx.0
            .send(RpcEvent::WebrtcRemoteIce(body.0, res))
            .await
            .map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        Ok(Json(WebrtcIceRemoteApiResponse {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }
}