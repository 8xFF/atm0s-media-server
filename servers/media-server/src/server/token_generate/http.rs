use std::sync::Arc;

use async_std::channel::Sender;
use cluster::{atm0s_sdn::Timer, SessionTokenSigner};
use poem::{web::Data, Result};
use poem_openapi::{
    param::Query,
    payload::Json,
    types::{ParseFromJSON, ToJSON, Type},
    Object, OpenApi,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct HttpContext {
    pub token: String,
    pub timer: Arc<dyn Timer>,
    pub signer: Arc<dyn SessionTokenSigner + Send + Sync>,
}

type DataContainer = (Sender<()>, HttpContext);

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct Response<D: ParseFromJSON + ToJSON + Type> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<D>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Object)]
pub struct TokenInfo {
    pub(crate) token: String,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Object)]
pub struct CreateRtmpSessionRequest {
    pub(crate) room: String,
    pub(crate) peer: String,
    pub(crate) expires_in: Option<u64>, //TODO check expire
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Object)]
pub struct CreateWhipSessionRequest {
    pub(crate) room: String,
    pub(crate) peer: String,
    pub(crate) expires_in: Option<u64>, //TODO check expire
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Object)]
pub struct CreateWhepSessionRequest {
    pub(crate) room: String,
    pub(crate) peer: Option<String>,
    pub(crate) expires_in: Option<u64>, //TODO check expire
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Object)]
pub struct CreateWebrtcSessionRequest {
    pub(crate) room: String,
    pub(crate) peer: Option<String>,
    pub(crate) publish: Option<bool>,
    pub(crate) subscribe: Option<bool>,
    pub(crate) expires_in: Option<u64>, //TODO check expire
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Object)]
pub struct CreateSipSessionRequest {
    pub(crate) peer: String,
    pub(crate) expires_in: Option<u64>, //TODO check expire
}

pub struct TokenGenerateHttpApis;

#[OpenApi]
impl TokenGenerateHttpApis {
    #[oai(path = "/app/rtmp_session", method = "post")]
    async fn create_rtmp_session(&self, Data(data): Data<&DataContainer>, app_secret: Query<String>, body: Json<CreateRtmpSessionRequest>) -> Result<Json<Response<TokenInfo>>> {
        if !app_secret.0.eq(&data.1.token) {
            return Ok(Json(Response {
                success: false,
                error: Some("INVALID_TOKEN".to_string()),
                data: None,
            }));
        }

        let token = data.1.signer.sign_media_session(&cluster::MediaSessionToken {
            room: Some(body.0.room),
            peer: Some(body.0.peer),
            protocol: cluster::rpc::general::MediaSessionProtocol::Rtmp,
            publish: true,
            subscribe: false,
            ts: data.1.timer.now_ms(),
        });
        Ok(Json(Response {
            success: true,
            error: None,
            data: Some(TokenInfo { token }),
        }))
    }

    #[oai(path = "/app/whip_session", method = "post")]
    async fn create_whip_session(&self, Data(data): Data<&DataContainer>, app_secret: Query<String>, body: Json<CreateWhipSessionRequest>) -> Result<Json<Response<TokenInfo>>> {
        if !app_secret.0.eq(&data.1.token) {
            return Ok(Json(Response {
                success: false,
                error: Some("INVALID_TOKEN".to_string()),
                data: None,
            }));
        }

        let token = data.1.signer.sign_media_session(&cluster::MediaSessionToken {
            room: Some(body.0.room),
            peer: Some(body.0.peer),
            protocol: cluster::rpc::general::MediaSessionProtocol::Whip,
            publish: true,
            subscribe: false,
            ts: data.1.timer.now_ms(),
        });
        Ok(Json(Response {
            success: true,
            error: None,
            data: Some(TokenInfo { token }),
        }))
    }

    #[oai(path = "/app/whep_session", method = "post")]
    async fn create_whep_session(&self, Data(data): Data<&DataContainer>, app_secret: Query<String>, body: Json<CreateWhepSessionRequest>) -> Result<Json<Response<TokenInfo>>> {
        if !app_secret.0.eq(&data.1.token) {
            return Ok(Json(Response {
                success: false,
                error: Some("INVALID_TOKEN".to_string()),
                data: None,
            }));
        }

        let token = data.1.signer.sign_media_session(&cluster::MediaSessionToken {
            room: Some(body.0.room),
            peer: body.0.peer,
            protocol: cluster::rpc::general::MediaSessionProtocol::Whep,
            publish: false,
            subscribe: true,
            ts: data.1.timer.now_ms(),
        });
        Ok(Json(Response {
            success: true,
            error: None,
            data: Some(TokenInfo { token }),
        }))
    }

    #[oai(path = "/app/webrtc_session", method = "post")]
    async fn create_webrtc_session(&self, Data(data): Data<&DataContainer>, app_secret: Query<String>, body: Json<CreateWebrtcSessionRequest>) -> Result<Json<Response<TokenInfo>>> {
        if !app_secret.0.eq(&data.1.token) {
            return Ok(Json(Response {
                success: false,
                error: Some("INVALID_TOKEN".to_string()),
                data: None,
            }));
        }

        let token = data.1.signer.sign_media_session(&cluster::MediaSessionToken {
            room: Some(body.0.room),
            peer: body.0.peer,
            protocol: cluster::rpc::general::MediaSessionProtocol::Webrtc,
            publish: body.0.publish.unwrap_or(true),
            subscribe: body.0.subscribe.unwrap_or(true),
            ts: data.1.timer.now_ms(),
        });
        Ok(Json(Response {
            success: true,
            error: None,
            data: Some(TokenInfo { token }),
        }))
    }

    #[oai(path = "/app/sip_session", method = "post")]
    async fn create_sip_session(&self, Data(data): Data<&DataContainer>, app_secret: Query<String>, body: Json<CreateSipSessionRequest>) -> Result<Json<Response<TokenInfo>>> {
        if !app_secret.0.eq(&data.1.token) {
            return Ok(Json(Response {
                success: false,
                error: Some("INVALID_TOKEN".to_string()),
                data: None,
            }));
        }

        let token = data.1.signer.sign_media_session(&cluster::MediaSessionToken {
            room: None,
            peer: Some(body.0.peer),
            protocol: cluster::rpc::general::MediaSessionProtocol::Sip,
            publish: true,
            subscribe: true,
            ts: data.1.timer.now_ms(),
        });
        Ok(Json(Response {
            success: true,
            error: None,
            data: Some(TokenInfo { token }),
        }))
    }
}
