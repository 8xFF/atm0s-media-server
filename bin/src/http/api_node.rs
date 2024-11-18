use poem_openapi::{payload::Json, OpenApi};

use super::Response;

pub struct NodeApiCtx {
    pub address: String,
}

pub struct Apis {
    ctx: NodeApiCtx,
}

impl Apis {
    pub fn new(ctx: NodeApiCtx) -> Self {
        Self { ctx }
    }
}

#[OpenApi]
impl Apis {
    #[oai(path = "/address", method = "get")]
    async fn get_address(&self) -> Json<Response<String>> {
        Json(Response {
            status: true,
            data: Some(self.ctx.address.clone()),
            ..Default::default()
        })
    }
}
