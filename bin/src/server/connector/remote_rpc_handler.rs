use std::sync::Arc;

use media_server_connector::{sql_storage, Querier};
use media_server_protocol::protobuf::cluster_connector::{get_events::EventInfo, GetEvents, GetParams, MediaConnectorServiceHandler};

#[derive(Clone)]
pub struct Ctx {
    pub storage: Arc<sql_storage::ConnectorStorage>, //TODO make it generic
}

#[derive(Default)]
pub struct ConnectorRemoteRpcHandlerImpl {}

impl MediaConnectorServiceHandler<Ctx> for ConnectorRemoteRpcHandlerImpl {
    async fn events(&self, ctx: &Ctx, req: GetParams) -> Option<GetEvents> {
        let events = ctx
            .storage
            .events(None, req.page as usize, req.limit as usize)
            .await?
            .into_iter()
            .map(|e| EventInfo {
                id: e.id,
                node: e.node,
                node_ts: e.node_ts,
                session: e.session,
                created_at: e.created_at,
                event: e.event,
                meta: e.meta.map(|m| m.to_string()),
            })
            .collect::<Vec<_>>();
        log::info!("{:?}", events);
        Some(GetEvents { events })
    }
}
