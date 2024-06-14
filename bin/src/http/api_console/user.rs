use super::{super::Response, ConsoleApisCtx};
use poem::web::Data;
use poem_openapi::{payload::Json, OpenApi};

#[derive(poem_openapi::Object)]
pub struct UserLoginReq {
    pub user: String,
    pub password: String,
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
        //TODO implement token and db
        Json(Response {
            status: true,
            error: None,
            data: Some(UserLoginRes { token: "this-is-token".to_string() }),
        })
    }
}
