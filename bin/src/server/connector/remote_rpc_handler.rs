use media_server_protocol::protobuf::cluster_connector::{Empty, MediaConnectorServiceHandler};

#[derive(Clone)]
pub struct Ctx {}

#[derive(Default)]
pub struct ConnectorRemoteRpcHandlerImpl {}

impl MediaConnectorServiceHandler<Ctx> for ConnectorRemoteRpcHandlerImpl {
    async fn hello(&self, ctx: &Ctx, req: Empty) -> Option<Empty> {
        None
    }
}
