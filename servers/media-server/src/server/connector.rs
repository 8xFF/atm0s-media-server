use clap::Parser;
use cluster::{
    rpc::{
        connector::MediaEndpointLogResponse,
        general::{NodeInfo, ServerType},
        RpcEmitter, RpcEndpoint, RpcRequest,
    },
    Cluster, ClusterEndpoint,
};
use futures::{select, FutureExt};
use metrics_dashboard::build_dashboard_route;
use poem::{web::Json, Route};
use poem_openapi::OpenApiService;
use prost::Message;
use protocol::media_event_logs::MediaEndpointLogRequest;

use crate::rpc::http::HttpRpcServer;

mod queue;
mod rpc;
mod transports;

use self::{
    queue::TransporterQueue,
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

    /// Filebase backup path for logs
    #[arg(env, long, default_value = ".atm0s/data/connector-queue")]
    backup_path: String,

    /// Max conn
    #[arg(env, long, default_value_t = 100)]
    pub max_conn: u64,
}

pub async fn run_connector_server<C, CR, RPC, REQ, EMITTER>(
    http_port: u16,
    http_tls: bool,
    opts: ConnectorArgs,
    ctx: MediaServerContext<InternalControl>,
    cluster: C,
    rpc_endpoint: RPC,
) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let mut rpc_endpoint = ConnectorClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port, http_tls);
    let (protocol, _) = parse_uri(&opts.mq_uri).map_err(|e| {
        log::error!("Error parsing MQ URI: {:?}", e);
        "Error parsing MQ URI"
    })?;

    let transporter: Box<dyn ConnectorTransporter<MediaEndpointLogRequest>> = match protocol.as_str() {
        "nats" => {
            let nats = NatsTransporter::<MediaEndpointLogRequest>::new(opts.mq_uri.clone(), opts.mq_channel.clone())
                .await
                .expect("Nats should be connected");
            Box::new(nats)
        }
        _ => {
            log::error!("Unsupported transporter");
            return Err("Unsupported transporter");
        }
    };
    let (mut transporter_queue, mut queue_sender) = match TransporterQueue::new(&opts.backup_path, transporter) {
        Ok((queue, tx)) => (queue, tx),
        Err(err) => {
            log::error!("Error creating queue: {:?}", err);
            return Err("Error creating queue");
        }
    };

    async_std::task::spawn(async move {
        loop {
            if let Err(e) = transporter_queue.poll().await {
                log::error!("msg queue transport error {:?}", e);
                panic!("msg queue transport error, should restart");
            }
        }
    });

    let node_info = NodeInfo {
        node_id: cluster.node_id(),
        address: format!("{}", cluster.node_addr()),
        server_type: ServerType::CONNECTOR,
    };

    let api_service = OpenApiService::new(ConnectorHttpApis, "Connector Server", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/node-info/", poem::endpoint::make_sync(move |_| Json(node_info.clone())))
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

                let data = req.param();

                if let Err(e) = queue_sender.try_send(data.encode_to_vec()) {
                    log::error!("Error sending message: {:?}", e);
                }

                req.answer(Ok(MediaEndpointLogResponse {}));
            }
        }
    }
}
