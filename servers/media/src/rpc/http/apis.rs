use async_std::channel::Sender;
use media_utils::Response;
use poem::{
    http::StatusCode,
    web::{Data, Path},
    Result,
};
use poem_openapi::{payload::Json, Object, OpenApi};
use serde::{Deserialize, Serialize};
use transport::RpcResponse;
use transport_webrtc::{WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse, WhepConnectResponse, WhipConnectResponse};

use crate::rpc::RpcEvent;

use super::payload_sdp::{ApplicationSdp, HttpResponse};

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct WebrtcSdp {
    pub node_id: u32,
    pub conn_id: String,
    pub sdp: String,
    /// This is use for provide proof of Price
    pub service_token: Option<String>,
}

pub type WhipDestroyApiResponse = Response<String>;
pub type WhepDestroyApiResponse = Response<String>;
pub type WebrtcConnectApiResponse = Response<WebrtcSdp>;
pub type WebrtcIceRemoteApiResponse = Response<WebrtcRemoteIceResponse>;

pub struct HttpApis;

#[OpenApi]
impl HttpApis {
    /// connect whip endpoint
    #[oai(path = "/api/whip/endpoint", method = "post")]
    async fn create_whip(&self, ctx: Data<&Sender<RpcEvent>>, body: ApplicationSdp<String>) -> Result<HttpResponse<ApplicationSdp<String>>> {
        log::info!("[HttpApis] create whip endpoint with sdp {}", body.0);
        let (res, rx) = RpcResponse::<WhipConnectResponse>::new();
        ctx.0
            .send(RpcEvent::WhipConnect("token".to_string(), body.0, res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whip endpoint created with location {} and sdp {}", res.location, res.sdp);
        Ok(HttpResponse {
            code: StatusCode::CREATED,
            res: ApplicationSdp(res.sdp),
            headers: vec![("location", res.location)],
        })
    }

    /// patch whip conn for trickle-ice
    #[oai(path = "/api/whip/conn/:conn_id", method = "patch")]
    async fn conn_whip_patch(&self, ctx: Data<&Sender<RpcEvent>>, conn_id: Path<String>, body: ApplicationSdp<String>) -> Result<ApplicationSdp<String>> {
        log::info!("[HttpApis] patch whip endpoint with sdp {}", body.0);
        let (res, rx) = RpcResponse::<String>::new();
        ctx.0
            .send(RpcEvent::WhipPatch(conn_id.0, body.0, res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whip endpoint patch with sdp {}", res);
        Ok(ApplicationSdp(res))
    }

    // /// post whip conn for action
    // #[oai(path = "/api/whip/conn/:conn_id", method = "post")]
    // async fn conn_whip_patch(&self, ctx: Data<&Sender<RpcEvent>>, conn_id: Path<String>, body: Json<String>) -> Result<ApplicationSdp<String>> {
    //     todo!()
    // }

    /// delete whip conn
    #[oai(path = "/api/whip/conn/:conn_id", method = "delete")]
    async fn conn_whip_delete(&self, ctx: Data<&Sender<RpcEvent>>, conn_id: Path<String>) -> Result<Json<WhipDestroyApiResponse>> {
        log::info!("[HttpApis] close whip endpoint conn {}", conn_id.0);
        let (res, rx) = RpcResponse::<()>::new();
        ctx.0
            .send(RpcEvent::WhipClose(conn_id.0.clone(), res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let _res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whip endpoint closed conn {}", conn_id.0);
        Ok(Json(WhipDestroyApiResponse {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }

    /// connect whep endpoint
    #[oai(path = "/api/whep/endpoint", method = "post")]
    async fn create_whep(&self, ctx: Data<&Sender<RpcEvent>>, body: ApplicationSdp<String>) -> Result<HttpResponse<ApplicationSdp<String>>> {
        log::info!("[HttpApis] create whep endpoint with sdp {}", body.0);
        let (res, rx) = RpcResponse::<WhepConnectResponse>::new();
        ctx.0
            .send(RpcEvent::WhepConnect("token".to_string(), body.0, res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whep endpoint created with location {} and sdp {}", res.location, res.sdp);
        Ok(HttpResponse {
            code: StatusCode::CREATED,
            res: ApplicationSdp(res.sdp),
            headers: vec![("location", res.location)],
        })
    }

    /// patch whep conn for trickle-ice
    #[oai(path = "/api/whep/conn/:conn_id", method = "patch")]
    async fn conn_whep_patch(&self, ctx: Data<&Sender<RpcEvent>>, conn_id: Path<String>, body: ApplicationSdp<String>) -> Result<ApplicationSdp<String>> {
        log::info!("[HttpApis] patch whep endpoint with sdp {}", body.0);
        let (res, rx) = RpcResponse::<String>::new();
        ctx.0
            .send(RpcEvent::WhepPatch(conn_id.0, body.0, res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whep endpoint patch with sdp {}", res);
        Ok(ApplicationSdp(res))
    }

    // /// post whep conn for action
    // #[oai(path = "/api/whep/conn/:conn_id", method = "post")]
    // async fn conn_whep_patch(&self, ctx: Data<&Sender<RpcEvent>>, conn_id: Path<String>, body: Json<String>) -> Result<ApplicationSdp<String>> {
    //     todo!()
    // }

    /// delete whip conn
    #[oai(path = "/api/whep/conn/:conn_id", method = "delete")]
    async fn conn_whep_delete(&self, ctx: Data<&Sender<RpcEvent>>, conn_id: Path<String>) -> Result<Json<WhepDestroyApiResponse>> {
        log::info!("[HttpApis] close whep endpoint conn {}", conn_id.0);
        let (res, rx) = RpcResponse::<()>::new();
        ctx.0
            .send(RpcEvent::WhepClose(conn_id.0.clone(), res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let _res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whep endpoint closed conn {}", conn_id.0);
        Ok(Json(WhipDestroyApiResponse {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }

    /// connect webrtc endpoint
    #[oai(path = "/api/webrtc/connect", method = "post")]
    async fn create_webrtc(&self, ctx: Data<&Sender<RpcEvent>>, body: Json<WebrtcConnectRequest>) -> Result<Json<WebrtcConnectApiResponse>> {
        log::info!("[HttpApis] create Webrtc endpoint {}/{}", body.0.room, body.0.peer);
        let (res, rx) = RpcResponse::<WebrtcConnectResponse>::new();
        ctx.0
            .send(RpcEvent::WebrtcConnect(body.0, res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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
    #[oai(path = "/api/webrtc/ice_remote", method = "post")]
    async fn webrtc_ice_remote(&self, ctx: Data<&Sender<RpcEvent>>, body: Json<WebrtcRemoteIceRequest>) -> Result<Json<WebrtcIceRemoteApiResponse>> {
        log::info!("[HttpApis] on Webrtc endpoint ice-remote {}/{:?}", body.0.candidate, body.0.sdp_mid);
        let (res, rx) = RpcResponse::<()>::new();
        ctx.0
            .send(RpcEvent::WebrtcRemoteIce(body.0, res))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let (_code, res) = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        Ok(Json(WebrtcIceRemoteApiResponse {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }
}
