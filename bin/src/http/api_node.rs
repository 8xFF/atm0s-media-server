use std::time::Duration;

use atm0s_sdn::NodeAddr;
use poem_openapi::{
    payload::{Json, PlainText},
    OpenApi,
};
use tokio::sync::{mpsc::Sender, oneshot};

pub struct NodeApiCtx {
    pub address: NodeAddr,
    pub dump_tx: Sender<oneshot::Sender<serde_json::Value>>,
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

    #[oai(path = "/router_dump", method = "get")]
    async fn get_router_dump(&self) -> Json<serde_json::Value> {
        let (tx, rx) = oneshot::channel();
        self.ctx.dump_tx.send(tx).await.expect("should send");
        match tokio::time::timeout(Duration::from_millis(1000), rx).await {
            Ok(Ok(v)) => Json(serde_json::json!({
                "status": true,
                "data": v
            })),
            Ok(Err(e)) => Json(serde_json::json!({
                "status": false,
                "error": e.to_string()
            })),
            Err(_e) => Json(serde_json::json!({
                "status": false,
                "error": "timeout"
            })),
        }
    }
}
