use atm0s_sdn::NodeAddr;
use poem_openapi::{payload::PlainText, OpenApi};

pub struct NodeApiCtx {
    pub address: NodeAddr,
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
    async fn get_address(&self) -> PlainText<String> {
        PlainText(self.ctx.address.to_string())
    }
}
