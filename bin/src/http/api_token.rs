use std::sync::Arc;

use super::{utils::TokenAuthorization, Response};
use media_server_protocol::tokens::{WebrtcToken, WhepToken, WhipToken};
use media_server_secure::{jwt::MediaGatewaySecureJwt, MediaGatewaySecure};
use poem::{web::Data, Result};
use poem_openapi::{payload::Json, OpenApi};

#[derive(Clone)]
pub struct TokenServerCtx {
    pub(crate) secure: Arc<MediaGatewaySecureJwt>,
}

pub struct TokenApis;

#[derive(poem_openapi::Object)]
struct WhipTokenReq {
    room: String,
    peer: String,
    ttl: u64,
}

#[derive(poem_openapi::Object)]
struct WhipTokenRes {
    token: String,
}

#[derive(poem_openapi::Object)]
struct WhepTokenReq {
    room: String,
    peer: Option<String>,
    ttl: u64,
}

#[derive(poem_openapi::Object)]
struct WhepTokenRes {
    token: String,
}

#[derive(poem_openapi::Object)]
struct WebrtcTokenReq {
    room: Option<String>,
    peer: Option<String>,
    ttl: u64,
}

#[derive(poem_openapi::Object)]
struct WebrtcTokenRes {
    token: String,
}

#[OpenApi]
impl TokenApis {
    #[oai(path = "/demo", method = "get")]
    async fn demo(&self) -> Result<Json<Response<WhipTokenRes>>> {
        todo!()
    }

    /// create whip session token
    #[oai(path = "/whip", method = "post")]
    async fn whip_token(&self, Data(ctx): Data<&TokenServerCtx>, body: Json<WhipTokenReq>, TokenAuthorization(token): TokenAuthorization) -> Result<Json<Response<WhipTokenRes>>> {
        if ctx.secure.validate_app(&token.token) {
            let body = body.0;
            Ok(Json(Response {
                status: true,
                data: Some(WhipTokenRes {
                    token: ctx.secure.encode_obj("whip", WhipToken { room: body.room, peer: body.peer }, body.ttl),
                }),
                error: None,
            }))
        } else {
            Ok(Json(Response {
                status: false,
                error: Some("APP_TOKEN_INVALID".to_string()),
                data: None,
            }))
        }
    }

    /// create whep session token
    #[oai(path = "/whep", method = "post")]
    async fn whep_token(&self, Data(ctx): Data<&TokenServerCtx>, body: Json<WhepTokenReq>, TokenAuthorization(token): TokenAuthorization) -> Json<Response<WhepTokenRes>> {
        if ctx.secure.validate_app(&token.token) {
            let body = body.0;
            Json(Response {
                status: true,
                data: Some(WhepTokenRes {
                    token: ctx.secure.encode_obj("whep", WhepToken { room: body.room, peer: body.peer }, body.ttl),
                }),
                error: None,
            })
        } else {
            Json(Response {
                status: false,
                error: Some("APP_TOKEN_INVALID".to_string()),
                data: None,
            })
        }
    }

    #[oai(path = "/webrtc", method = "post")]
    async fn webrtc_token(&self, Data(ctx): Data<&TokenServerCtx>, body: Json<WebrtcTokenReq>, TokenAuthorization(token): TokenAuthorization) -> Json<Response<WebrtcTokenRes>> {
        if ctx.secure.validate_app(&token.token) {
            let body = body.0;
            Json(Response {
                status: true,
                data: Some(WebrtcTokenRes {
                    token: ctx.secure.encode_obj("webrtc", WebrtcToken { room: body.room, peer: body.peer }, body.ttl),
                }),
                error: None,
            })
        } else {
            Json(Response {
                status: false,
                error: Some("APP_TOKEN_INVALID".to_string()),
                data: None,
            })
        }
    }
}
