use clap::Parser;
use cluster::{
    rpc::{connector::MediaEndpointLogResponse, RpcEmitter, RpcEndpoint, RpcRequest},
    Cluster, ClusterEndpoint,
};
use futures::{select, FutureExt};
use metrics_dashboard::build_dashboard_route;
use poem::Route;
use poem_openapi::OpenApiService;
use protocol::media_event_logs::MediaEndpointLogRequest;

use crate::rpc::http::HttpRpcServer;

mod rpc;
mod transports;

use self::{
    rpc::{cluster::ConnectorClusterRpc, http::ConnectorHttpApis, InternalControl, RpcEvent},
    transports::nats::NatsTransporter,
    transports::{parse_uri, ConnectorTransporter},
};

use super::MediaServerContext;

/// Media Server Webrtc
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct ConnectorArgs {
    /// Message Queue URI in the form of `amqp://user:pass@host:port/vhost`
    #[arg(env, long, default_value = "nats://localhost:4222")]
    mq_uri: String,

    /// MQ Channel
    #[arg(env, long, default_value = "atm0s/event_log")]
    mq_channel: String,

    /// Max conn
    #[arg(env, long, default_value_t = 100)]
    pub max_conn: u64,
}

pub async fn run_connector_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, _opts: ConnectorArgs, ctx: MediaServerContext<InternalControl>, _cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let mut rpc_endpoint = ConnectorClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port);
    let (protocol, _) = parse_uri(&_opts.mq_uri).map_err(|e| {
        log::error!("Error parsing MQ URI: {:?}", e);
        "Error parsing MQ URI"
    })?;
    let transporter: Result<Box<dyn ConnectorTransporter<MediaEndpointLogRequest>>, String> = match protocol.as_str() {
        "nats" => {
            let nats = NatsTransporter::new(_opts.mq_uri.clone(), _opts.mq_channel.clone()).await;
            match nats {
                Ok(nats) => Ok(Box::new(nats)),
                Err(e) => {
                    log::error!("Error creating Nats transporter: {:?}", e);
                    return Err("Error creating Nats transporter");
                }
            }
        }
        _ => {
            log::error!("Unsupported transporter");
            return Err("Unsupported transporter");
        }
    };

    let api_service = OpenApiService::new(ConnectorHttpApis, "Connector Server", "1.0.0").server(format!("http://localhost:{}", http_port));
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()));

    http_server.start(route, ctx).await;

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
                if let Ok(ref transport) = transporter {
                    let data = req.param();

                    if let Err(e) = transport.send(data).await {
                        log::error!("Error sending message: {:?}", e);
                    }
                }
                req.answer(Ok(MediaEndpointLogResponse {}));
            }
        }
    }
}
