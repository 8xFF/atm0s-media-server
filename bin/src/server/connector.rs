use std::{sync::Arc, time::Duration};

use atm0s_sdn::{features::FeaturesEvent, secure::StaticKeyAuthorization, services::visualization, SdnBuilder, SdnControllerUtils, SdnExtOut, SdnOwner};
use clap::Parser;
use media_server_connector::{
    handler_service::{self, ConnectorHandlerServiceBuilder},
    hooks::{ConnectorHookController, HookControllerCfg, HookPublisher},
    sql_storage::ConnectorStorage,
    Storage, HANDLER_SERVICE_ID,
};
use media_server_protocol::{
    cluster::{ClusterNodeGenericInfo, ClusterNodeInfo},
    connector::CONNECTOR_RPC_PORT,
    protobuf::cluster_connector::{connector_response, MediaConnectorServiceServer},
    rpc::quinn::QuinnServer,
};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use tokio::{select, sync::mpsc::channel, time};

use crate::{
    node_metrics::NodeMetricsCollector,
    quinn::{make_quinn_server, VirtualNetwork},
    NodeConfig,
};
use sans_io_runtime::backend::PollingBackend;

mod http_hook_publisher;
mod remote_rpc_handler;

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SC {
    Visual(visualization::Control<ClusterNodeInfo>),
    Connector(media_server_connector::handler_service::Control),
}

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SE {
    Visual(visualization::Event<ClusterNodeInfo>),
    Connector(media_server_connector::handler_service::Event),
}
type TC = ();
type TW = ();

#[derive(Debug, Parser)]
pub struct Args {
    /// DB Uri
    #[arg(env, long, default_value = "sqlite://connector.db?mode=rwc")]
    db_uri: String,

    /// S3 Uri
    #[arg(env, long, default_value = "http://user:pass@localhost:9000/bucket/path/?path_style=true")]
    s3_uri: String,

    /// Hook Uri.
    /// If set, will send hook event to this uri. example: http://localhost:8080/hook
    #[arg(env, long)]
    hook_uri: Option<String>,
}

pub async fn run_media_connector(workers: usize, node: NodeConfig, args: Args) {
    rustls::crypto::ring::default_provider().install_default().expect("should install ring as default");

    let connector_storage = Arc::new(ConnectorStorage::new(&args.db_uri, &args.s3_uri).await);
    let hook_publisher: Option<Arc<dyn HookPublisher>> = if let Some(hook_uri) = args.hook_uri {
        log::info!("[Connector] Hook publisher enabled with uri {}", hook_uri);
        Some(Arc::new(http_hook_publisher::HttpHookPublisher::new(hook_uri)))
    } else {
        None
    };
    let mut hook_controller = ConnectorHookController::new(hook_publisher, HookControllerCfg { worker_num: 5, job_num: 10 });

    let default_cluster_cert_buf = include_bytes!("../../certs/cluster.cert");
    let default_cluster_key_buf = include_bytes!("../../certs/cluster.key");
    let default_cluster_cert = CertificateDer::from(default_cluster_cert_buf.to_vec());
    let default_cluster_key = PrivatePkcs8KeyDer::from(default_cluster_key_buf.to_vec());

    let node_id = node.node_id;

    let mut builder = SdnBuilder::<(), SC, SE, TC, TW, ClusterNodeInfo>::new(node_id, &node.bind_addrs, node.bind_addrs_alt);
    let node_addr = builder.node_addr();
    let node_info = ClusterNodeInfo::Connector(ClusterNodeGenericInfo {
        addr: node_addr.to_string(),
        cpu: 0,
        memory: 0,
        disk: 0,
    });

    builder.set_authorization(StaticKeyAuthorization::new(&node.secret));
    builder.set_manual_discovery(vec!["connector".to_string()], vec!["gateway".to_string()]);
    builder.add_service(Arc::new(ConnectorHandlerServiceBuilder::new()));

    for seed in node.seeds {
        builder.add_seed(seed);
    }

    let mut controller = builder.build::<PollingBackend<SdnOwner, 128, 128>>(workers, node_info);

    //
    // Vnet is a virtual udp layer for creating RPC handlers, we separate media server to 2 layer
    // - async for business logic like proxy, logging handling
    // - sync with sans-io style for media data
    //
    let (mut vnet, vnet_tx, mut vnet_rx) = VirtualNetwork::new(node.node_id);

    let media_rpc_socket = vnet.udp_socket(CONNECTOR_RPC_PORT).await.expect("Should open virtual port for gateway rpc");
    let mut media_rpc_server = MediaConnectorServiceServer::new(
        QuinnServer::new(make_quinn_server(media_rpc_socket, default_cluster_key, default_cluster_cert.clone()).expect("Should create endpoint for media rpc server")),
        remote_rpc_handler::Ctx { storage: connector_storage.clone() },
        remote_rpc_handler::ConnectorRemoteRpcHandlerImpl::default(),
    );

    tokio::task::spawn_local(async move {
        media_rpc_server.run().await;
    });

    tokio::task::spawn_local(async move { while vnet.recv().await.is_some() {} });

    // Collect node metrics for update to gateway agent service, this information is used inside gateway
    // for forwarding from other gateway
    let mut node_metrics_collector = NodeMetricsCollector::default();

    // Subscribe ConnectorHandler service
    controller.service_control(HANDLER_SERVICE_ID.into(), (), handler_service::Control::Sub.into());

    let (connector_storage_tx, mut connector_storage_rx) = channel(1024);
    let (connector_hook_tx, mut connector_hook_rx) = channel(1024);
    let (connector_handler_control_tx, mut connector_handler_control_rx) = channel(1024);
    tokio::task::spawn_local(async move {
        while let Some((from, ts, req_id, event)) = connector_storage_rx.recv().await {
            match connector_storage.on_event(from, ts, event).await {
                Some(res) => {
                    if let Err(e) = connector_handler_control_tx.send(handler_service::Control::Res(from, req_id, res)).await {
                        log::error!("[Connector] send control to service error {:?}", e);
                    }
                }
                None => {
                    if let Err(e) = connector_handler_control_tx
                        .send(handler_service::Control::Res(
                            from,
                            req_id,
                            connector_response::Response::Error(connector_response::Error {
                                code: 0, //TODO return error from storage
                                message: "STORAGE_ERROR".to_string(),
                            }),
                        ))
                        .await
                    {
                        log::error!("[Connector] send control to service error {:?}", e);
                    }
                }
            }
        }
    });

    tokio::task::spawn_local(async move {
        let mut interval = time::interval(Duration::from_millis(10));
        loop {
            select! {
                Some((from, ts, _req_id, req)) = connector_hook_rx.recv() => {
                    hook_controller.on_event(from, ts, req);
                }
                _ = interval.tick()  => {
                    hook_controller.on_tick().await;
                }
                else => {
                    break;
                }
            }
        }
    });

    loop {
        if controller.process().is_none() {
            break;
        }

        // Pop from metric collector and pass to Gateway store service
        if let Some(metrics) = node_metrics_collector.pop_measure() {
            let node_info = ClusterNodeInfo::Connector(ClusterNodeGenericInfo {
                addr: node_addr.to_string(),
                cpu: metrics.cpu,
                memory: metrics.memory,
                disk: metrics.disk,
            });
            controller.service_control(visualization::SERVICE_ID.into(), (), visualization::Control::UpdateInfo(node_info).into());
        }
        while let Ok(control) = vnet_rx.try_recv() {
            controller.feature_control((), control.into());
        }

        while let Ok(control) = connector_handler_control_rx.try_recv() {
            controller.service_control(HANDLER_SERVICE_ID.into(), (), control.into());
        }

        while let Some(out) = controller.pop_event() {
            match out {
                SdnExtOut::ServicesEvent(_, _, SE::Connector(event)) => match event {
                    media_server_connector::handler_service::Event::Req(from, ts, req_id, event) => {
                        let ev = event.clone();
                        if let Err(e) = connector_storage_tx.send((from, ts, req_id, event)).await {
                            log::error!("[MediaConnector] send event to storage error {:?}", e);
                        }
                        let _ = connector_hook_tx.send((from, ts, req_id, ev)).await;
                    }
                },
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::Socket(event)) => {
                    if let Err(e) = vnet_tx.try_send(event) {
                        log::error!("[MediaConnector] forward Sdn SocketEvent error {:?}", e);
                    }
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
