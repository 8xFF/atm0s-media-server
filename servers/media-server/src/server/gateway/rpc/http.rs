use std::ops::Deref;

use async_std::channel::Sender;
use cluster::rpc::general::MediaEndpointCloseRequest;
use cluster::rpc::general::MediaEndpointCloseResponse;
use cluster::rpc::webrtc::*;
use cluster::rpc::whep::*;
use cluster::rpc::whip::*;
use cluster::VerifyObject;
use media_utils::Response;
use media_utils::StringCompression;
use poem::{
    http::StatusCode,
    web::{Data, Path},
    Result,
};
use poem_openapi::payload::Response as HttpResponse;
use poem_openapi::{payload::Json, Object, OpenApi};
use serde::{Deserialize, Serialize};

use crate::rpc::http::CustomHttpResponse;
use crate::rpc::http::RemoteIpAddr;
use crate::rpc::http::TokenAuthorization;
use crate::rpc::http::UserAgent;
use crate::rpc::http::{ApplicationSdp, ApplicationSdpPatch, RpcReqResHttp};
use crate::server::MediaServerContext;

type DataContainer = (Sender<RpcEvent>, MediaServerContext<()>);

use super::RpcEvent;

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct WebrtcSdp {
    pub node_id: u32,
    pub conn_id: String,
    pub sdp: String,
}

pub struct GatewayHttpApis;

#[OpenApi]
impl GatewayHttpApis {
    /// get node health
    #[oai(path = "/health", method = "get")]
    async fn health(&self, Data(_ctx): Data<&DataContainer>) -> Result<Json<Response<String>>> {
        Ok(Json(Response {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }

    /// connect whip endpoint
    #[oai(path = "/whip/endpoint", method = "post")]
    async fn create_whip(
        &self,
        Data(data): Data<&DataContainer>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        let string_zip = StringCompression::default();
        let req = WhipConnectRequest {
            session_uuid: data.1.generate_session_uuid(),
            ip_addr,
            user_agent,
            token: token.token,
            sdp: None,
            compressed_sdp: Some(string_zip.compress(&body.0)),
        };

        let s_token = req.verify(data.1.verifier().deref()).ok_or(poem::Error::from_status(StatusCode::UNAUTHORIZED))?;
        log::info!("[HttpApis] create whip endpoint {:?}", s_token);
        let (req, rx) = RpcReqResHttp::<WhipConnectRequest, WhipConnectResponse>::new(req);
        data.0
            .send(RpcEvent::WhipConnect(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        let sdp = match (res.sdp, res.compressed_sdp) {
            (Some(sdp), _) => Ok(sdp),
            (_, Some(compressed_sdp)) => string_zip.uncompress(&compressed_sdp).ok_or(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }?;
        log::info!("[HttpApis] Whip endpoint created with conn_id {} and sdp {}", res.conn_id, sdp);
        Ok(CustomHttpResponse {
            code: StatusCode::CREATED,
            res: ApplicationSdp(sdp),
            headers: vec![("location", format!("/whip/conn/{}", res.conn_id))],
        })
    }

    /// patch whip conn for trickle-ice
    #[oai(path = "/whip/conn/:conn_id", method = "patch")]
    async fn conn_whip_patch(&self, Data(data): Data<&DataContainer>, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        log::info!("[HttpApis] patch whip endpoint with sdp {}", body.0);
        let (req, rx) = RpcReqResHttp::<WebrtcPatchRequest, WebrtcPatchResponse>::new(WebrtcPatchRequest { conn_id: conn_id.0, sdp: body.0 });
        data.0
            .send(RpcEvent::WebrtcSdpPatch(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        if let Some(sdp) = res.ice_restart_sdp {
            log::info!("[HttpApis] Whip endpoint patch with ice_restart");
            Ok(HttpResponse::new(ApplicationSdpPatch(sdp)))
        } else {
            log::info!("[HttpApis] Whip endpoint patch without ice_restart");
            Ok(HttpResponse::new(ApplicationSdpPatch("".to_string())).status(StatusCode::NO_CONTENT))
        }
    }

    // /// post whip conn for action
    // #[oai(path = "/api/whip/conn/:conn_id", method = "post")]
    // async fn conn_whip_patch(&self, Data(data): Data<&DataContainer>, conn_id: Path<String>, body: Json<String>) -> Result<ApplicationSdp<String>> {
    //     todo!()
    // }

    /// delete whip conn
    #[oai(path = "/whip/conn/:conn_id", method = "delete")]
    async fn conn_whip_delete(&self, Data(data): Data<&DataContainer>, conn_id: Path<String>) -> Result<Json<Response<String>>> {
        log::info!("[HttpApis] close whip endpoint conn {}", conn_id.0);
        let (req, rx) = RpcReqResHttp::<MediaEndpointCloseRequest, MediaEndpointCloseResponse>::new(MediaEndpointCloseRequest { conn_id: conn_id.0.clone() });
        data.0
            .send(RpcEvent::MediaEndpointClose(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let _res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whip endpoint closed conn {}", conn_id.0);
        Ok(Json(Response {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }

    /// connect whep endpoint
    #[oai(path = "/whep/endpoint", method = "post")]
    async fn create_whep(
        &self,
        Data(data): Data<&DataContainer>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        let string_zip = StringCompression::default();
        let req = WhepConnectRequest {
            session_uuid: data.1.generate_session_uuid(),
            ip_addr,
            user_agent,
            token: token.token,
            sdp: None,
            compressed_sdp: Some(string_zip.compress(&body.0)),
        };
        let s_token = req.verify(data.1.verifier().deref()).ok_or(poem::Error::from_status(StatusCode::UNAUTHORIZED))?;
        log::info!("[HttpApis] create whep endpoint {:?}", s_token);
        let (req, rx) = RpcReqResHttp::<WhepConnectRequest, WhepConnectResponse>::new(req);
        data.0
            .send(RpcEvent::WhepConnect(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        let sdp = match (res.sdp, res.compressed_sdp) {
            (Some(sdp), _) => Ok(sdp),
            (_, Some(compressed_sdp)) => string_zip.uncompress(&compressed_sdp).ok_or(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }?;
        log::info!("[HttpApis] Whep endpoint created with conn_id {}", res.conn_id);
        Ok(CustomHttpResponse {
            code: StatusCode::CREATED,
            res: ApplicationSdp(sdp),
            headers: vec![("location", format!("/whep/conn/{}", res.conn_id))],
        })
    }

    /// patch whep conn for trickle-ice
    #[oai(path = "/whep/conn/:conn_id", method = "patch")]
    async fn conn_whep_patch(&self, Data(data): Data<&DataContainer>, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        log::info!("[HttpApis] patch whep endpoint with sdp {}", body.0);
        let (req, rx) = RpcReqResHttp::<WebrtcPatchRequest, WebrtcPatchResponse>::new(WebrtcPatchRequest { conn_id: conn_id.0, sdp: body.0 });
        data.0
            .send(RpcEvent::WebrtcSdpPatch(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        if let Some(sdp) = res.ice_restart_sdp {
            log::info!("[HttpApis] Whep endpoint patch with ice_restart");
            Ok(HttpResponse::new(ApplicationSdpPatch(sdp)))
        } else {
            log::info!("[HttpApis] Whep endpoint patch without ice_restart");
            Ok(HttpResponse::new(ApplicationSdpPatch("".to_string())).status(StatusCode::NO_CONTENT))
        }
    }

    // /// post whep conn for action
    // #[oai(path = "/api/whep/conn/:conn_id", method = "post")]
    // async fn conn_whep_patch(&self, Data(data): Data<&DataContainer>, conn_id: Path<String>, body: Json<String>) -> Result<ApplicationSdp<String>> {
    //     todo!()
    // }

    /// delete whip conn
    #[oai(path = "/whep/conn/:conn_id", method = "delete")]
    async fn conn_whep_delete(&self, Data(data): Data<&DataContainer>, conn_id: Path<String>) -> Result<Json<Response<String>>> {
        log::info!("[HttpApis] close whep endpoint conn {}", conn_id.0);
        let (req, rx) = RpcReqResHttp::<MediaEndpointCloseRequest, MediaEndpointCloseResponse>::new(MediaEndpointCloseRequest { conn_id: conn_id.0.clone() });
        data.0
            .send(RpcEvent::MediaEndpointClose(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let _res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Whep endpoint closed conn {}", conn_id.0);
        Ok(Json(Response {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }

    /// connect webrtc endpoint
    #[oai(path = "/webrtc/connect", method = "post")]
    async fn create_webrtc(
        &self,
        Data(data): Data<&DataContainer>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        mut body: Json<WebrtcConnectRequest>,
    ) -> Result<Json<Response<WebrtcSdp>>> {
        let string_zip = StringCompression::default();
        log::info!("[HttpApis] create Webrtc endpoint {}/{}", body.0.room, body.0.peer);
        if let Some(sdp) = body.0.sdp.take() {
            body.0.compressed_sdp = Some(string_zip.compress(&sdp));
        }
        body.0.session_uuid = Some(data.1.generate_session_uuid());
        body.0.ip_addr = Some(ip_addr);
        body.0.user_agent = Some(user_agent);

        if body.0.verify(data.1.verifier().deref()).is_none() {
            return Ok(Json(Response {
                status: false,
                error: Some("INVALID_TOKEN".to_string()),
                data: None,
            }));
        }

        let (req, rx) = RpcReqResHttp::<WebrtcConnectRequest, WebrtcConnectResponse>::new(body.0);
        data.0
            .send(RpcEvent::WebrtcConnect(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        let sdp = match (res.sdp, res.compressed_sdp) {
            (Some(sdp), _) => Ok(sdp),
            (_, Some(compressed_sdp)) => string_zip.uncompress(&compressed_sdp).ok_or(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }?;
        log::info!("[HttpApis] Webrtc endpoint created with conn_id {}", res.conn_id);
        Ok(Json(Response {
            status: true,
            error: None,
            data: Some(WebrtcSdp {
                node_id: 0,
                conn_id: res.conn_id,
                sdp,
            }),
        }))
    }

    /// sending remote ice candidate
    #[oai(path = "/webrtc/ice_remote", method = "post")]
    async fn webrtc_ice_remote(&self, Data(data): Data<&DataContainer>, body: Json<WebrtcRemoteIceRequest>) -> Result<Json<Response<String>>> {
        log::info!("[HttpApis] on Webrtc endpoint ice-remote {}", body.0.candidate);
        let (req, rx) = RpcReqResHttp::<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>::new(body.0);
        data.0
            .send(RpcEvent::WebrtcRemoteIce(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        Ok(Json(Response {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }

    /// delete webrtc conn
    #[oai(path = "/webrtc/conn/:conn_id", method = "delete")]
    async fn conn_webrtc_delete(&self, Data(data): Data<&DataContainer>, conn_id: Path<String>) -> Result<Json<Response<String>>> {
        log::info!("[HttpApis] close webrtc endpoint conn {}", conn_id.0);
        let (req, rx) = RpcReqResHttp::<MediaEndpointCloseRequest, MediaEndpointCloseResponse>::new(MediaEndpointCloseRequest { conn_id: conn_id.0.clone() });
        data.0
            .send(RpcEvent::MediaEndpointClose(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let _res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Webrtc endpoint closed conn {}", conn_id.0);
        Ok(Json(Response {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }
}
