use super::{super::Response, ConsoleApisCtx};
use media_server_secure::MediaConsoleSecure;
use poem::web::Data;
use poem_openapi::{payload::Json, OpenApi};

#[derive(poem_openapi::Object)]
pub struct UserLoginReq {
    pub secret: String,
}

#[derive(poem_openapi::Object)]
pub struct UserLoginRes {
    pub token: String,
}

pub struct Apis;

#[OpenApi]
impl Apis {
    /// login with user credentials
    #[oai(path = "/user/login", method = "post")]
    async fn user_login(&self, Data(ctx): Data<&ConsoleApisCtx>, body: Json<UserLoginReq>) -> Json<Response<UserLoginRes>> {
        if ctx.secure.validate_secret(&body.secret) {
            Json(Response {
                status: true,
                data: Some(UserLoginRes { token: ctx.secure.generate_token() }),
                ..Default::default()
            })
        } else {
            Json(Response {
                status: false,
                error: Some("WRONG_SECRET".to_string()),
                ..Default::default()
            })
        }
    }
}
