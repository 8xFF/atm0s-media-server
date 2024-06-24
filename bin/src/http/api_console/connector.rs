use crate::server::console_storage::{Zone, ZoneDetails};

use super::{super::Response, ConsoleApisCtx, ConsoleAuthorization};
use media_server_protocol::{connector::CONNECTOR_RPC_PORT, protobuf::cluster_connector::GetParams, rpc::node_vnet_addr};
use poem::web::Data;
use poem_openapi::{
    param::{Path, Query},
    payload::Json,
    OpenApi,
};

#[derive(poem_openapi::Object)]
pub struct EventInfo {
    pub id: i32,
    pub session: u64,
    pub node: u32,
    pub node_ts: u64,
    pub created_at: u64,
    pub event: String,
    pub meta: Option<String>,
}

pub struct Apis;

#[OpenApi]
impl Apis {
    /// get events
    #[oai(path = "/:node/log/events", method = "get")]
    async fn events(&self, _auth: ConsoleAuthorization, Data(ctx): Data<&ConsoleApisCtx>, Path(node): Path<u32>, Query(page): Query<u32>, Query(limit): Query<u32>) -> Json<Response<Vec<EventInfo>>> {
        match ctx.connector.events(node_vnet_addr(node, CONNECTOR_RPC_PORT), GetParams { page, limit }).await {
            Some(res) => Json(Response {
                status: true,
                error: None,
                data: Some(
                    res.events
                        .into_iter()
                        .map(|e| EventInfo {
                            id: e.id,
                            session: e.session,
                            node: e.node,
                            node_ts: e.node_ts,
                            created_at: e.created_at,
                            event: e.event,
                            meta: e.meta,
                        })
                        .collect::<Vec<_>>(),
                ),
            }),
            None => Json(Response {
                status: false,
                error: Some("CLUSTER_ERROR".to_string()),
                data: None,
            }),
        }
    }
}
