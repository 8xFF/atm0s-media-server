use super::{super::Response, ConsoleApisCtx};
use poem::web::Data;
use poem_openapi::{payload::Json, OpenApi};

#[derive(poem_openapi::Object)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub secret: String,
    pub active: bool,
}

#[derive(poem_openapi::Object)]
pub struct CreateAppReq {
    pub name: String,
}

pub type CreateAppRes = AppInfo;

#[derive(poem_openapi::Object)]
pub struct UpdateAppReq {
    pub name: String,
    pub active: bool,
}

pub type UpdateAppRes = AppInfo;
pub type ResetAppSecretRes = AppInfo;

pub struct Apis;

#[OpenApi]
impl Apis {
    /// get apps
    #[oai(path = "/apps", method = "get")]
    async fn get_apps(&self, Data(ctx): Data<&ConsoleApisCtx>) -> Json<Response<Vec<AppInfo>>> {
        Json(Response {
            status: true,
            error: None,
            data: Some(vec![]),
        })
    }

    /// create app
    #[oai(path = "/apps", method = "post")]
    async fn create_app(&self, Data(ctx): Data<&ConsoleApisCtx>, body: Json<CreateAppReq>) -> Json<Response<CreateAppRes>> {
        todo!()
    }

    /// update app
    #[oai(path = "/apps/:id", method = "post")]
    async fn update_app(&self, Data(ctx): Data<&ConsoleApisCtx>, id: String, body: Json<UpdateAppReq>) -> Json<Response<CreateAppRes>> {
        todo!()
    }

    /// reset secrent
    #[oai(path = "/apps/:id/reset_secret", method = "post")]
    async fn reset_secret_app(&self, Data(ctx): Data<&ConsoleApisCtx>, id: String) -> Json<Response<CreateAppRes>> {
        todo!()
    }
}
