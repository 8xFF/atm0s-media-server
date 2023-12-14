use clap::Parser;
use cluster::{
    rpc::{connector::MediaEndpointLogResponse, RpcEmitter, RpcEndpoint, RpcRequest},
    Cluster, ClusterEndpoint,
};
use futures::{select, FutureExt};
use metrics_dashboard::build_dashboard_route;
use poem::Route;
use poem_openapi::OpenApiService;

use crate::rpc::http::HttpRpcServer;

mod rpc;

use self::rpc::{cluster::ConnectorClusterRpc, http::ConnectorHttpApis, RpcEvent};

/// Media Server Webrtc
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct ConnectorArgs {}

pub async fn run_connector_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, _opts: ConnectorArgs, _cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let mut rpc_endpoint = ConnectorClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port);

    let api_service = OpenApiService::new(ConnectorHttpApis, "Connector Server", "1.0.0").server("http://localhost:3000");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()));

    http_server.start(route).await;

    loop {
        let rpc = select! {
            rpc = http_server.recv().fuse() => {
                rpc.ok_or("HTTP_SERVER_ERROR")?
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc.ok_or("CLUSTER_RPC_ERROR")?
            }
        };

        match rpc {
            RpcEvent::MediaEndpointLog(req) => {
                log::info!("On media endpoint log {:?}", req.param());
                req.answer(Ok(MediaEndpointLogResponse {}));
            }
        }
    }
}
