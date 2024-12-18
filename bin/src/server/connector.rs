use std::{sync::Arc, time::Duration};

use atm0s_sdn::{
    features::{router_sync, FeaturesEvent},
    secure::StaticKeyAuthorization,
    services::visualization,
    SdnBuilder, SdnControllerUtils, SdnExtOut, SdnOwner,
};
use clap::Parser;
use media_server_connector::{
    handler_service::{self, ConnectorHandlerServiceBuilder},
    ConnectorCfg, ConnectorStorage, HookBodyType, HANDLER_SERVICE_ID,
};
use media_server_multi_tenancy::{MultiTenancyStorage, MultiTenancySync};
use media_server_protocol::{
    cluster::{ClusterNodeGenericInfo, ClusterNodeInfo},
    connector::CONNECTOR_RPC_PORT,
    gateway::generate_gateway_zone_tag,
    protobuf::cluster_connector::{connector_response, MediaConnectorServiceServer},
    rpc::quinn::QuinnServer,
};
use media_server_utils::select2;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use tokio::sync::mpsc::channel;

use crate::{
    http::{run_connector_http_server, NodeApiCtx},
    node_metrics::NodeMetricsCollector,
    quinn::{make_quinn_server, VirtualNetwork},
    NodeConfig,
};
use sans_io_runtime::backend::PollingBackend;

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
    pub db_uri: String,

    /// S3 Uri
    #[arg(env, long, default_value = "http://user:pass@localhost:9000/bucket/path/?path_style=true")]
    pub s3_uri: String,

    /// Hook Uri.
    /// If set, will send hook event to this uri. example: http://localhost:8080/hook
    #[arg(env, long)]
    pub hook_uri: Option<String>,

    /// Hook workers
    #[arg(env, long, default_value_t = 8)]
    pub hook_workers: usize,

    /// Hook body type
    #[arg(env, long, default_value = "protobuf-json")]
    pub hook_body_type: HookBodyType,

    /// Destroy room after no-one online, default is 2 minutes
    #[arg(env, long, default_value_t = 120_000)]
    pub destroy_room_after_ms: u64,

    /// Storage tick interval, default is 1 minute
    /// This is used for clearing ended room
    #[arg(env, long, default_value_t = 60_000)]
    pub storage_tick_interval_ms: u64,

    /// multi-tenancy sync endpoint
    #[arg(env, long)]
    pub multi_tenancy_sync: Option<String>,

    /// multi-tenancy sync endpoint
    #[arg(env, long, default_value_t = 30_000)]
    pub multi_tenancy_sync_interval_ms: u64,
}

pub async fn run_media_connector(workers: usize, http_port: Option<u16>, node: NodeConfig, args: Args) {
    let app_storage = if let Some(url) = args.multi_tenancy_sync {
        let app_storage = Arc::new(MultiTenancyStorage::new());
        let mut app_sync = MultiTenancySync::new(app_storage.clone(), url, Duration::from_millis(args.multi_tenancy_sync_interval_ms));
        tokio::spawn(async move {
            app_sync.run_loop().await;
        });
        app_storage
    } else {
        Arc::new(MultiTenancyStorage::new_with_single(&node.secret, args.hook_uri.as_deref()))
    };

    let mut connector_storage = ConnectorStorage::new(
        node.node_id,
        app_storage,
        ConnectorCfg {
            sql_uri: args.db_uri,
            s3_uri: args.s3_uri,
            hook_workers: args.hook_workers,
            hook_body_type: args.hook_body_type,
            room_destroy_after_ms: args.destroy_room_after_ms,
        },
    )
    .await;
    let connector_querier = connector_storage.querier();

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
    builder.set_manual_discovery(vec!["connector".to_string()], vec![generate_gateway_zone_tag(node.zone)]);
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
        remote_rpc_handler::Ctx { storage: Arc::new(connector_querier) },
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
    let (connector_handler_control_tx, mut connector_handler_control_rx) = channel(1024);
    let mut connector_storage_interval = tokio::time::interval(Duration::from_millis(args.storage_tick_interval_ms));
    tokio::task::spawn_local(async move {
        loop {
            match select2::or(connector_storage_interval.tick(), connector_storage_rx.recv()).await {
                select2::OrOutput::Left(_) => {
                    connector_storage.on_tick().await;
                }
                select2::OrOutput::Right(Some((from, ts, req_id, event))) => {
                    match connector_storage.on_event(from, ts, req_id, event).await {
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
                select2::OrOutput::Right(None) => {
                    break;
                }
            }
        }
    });

    // Start http server
    let (dump_tx, mut dump_rx) = channel(10);
    if let Some(http_port) = http_port {
        let node_ctx = NodeApiCtx { address: node_addr.clone(), dump_tx };
        tokio::spawn(async move {
            if let Err(e) = run_connector_http_server(http_port, node_ctx).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }
    let mut wait_dump_router = vec![];

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

        while let Ok(v) = dump_rx.try_recv() {
            controller.feature_control((), router_sync::Control::DumpRouter.into());
            wait_dump_router.push(v);
        }

        while let Some(out) = controller.pop_event() {
            match out {
                SdnExtOut::ServicesEvent(_, _, SE::Connector(event)) => match event {
                    media_server_connector::handler_service::Event::Req(from, ts, req_id, event) => {
                        if let Err(e) = connector_storage_tx.send((from, ts, req_id, event)).await {
                            log::error!("[MediaConnector] send event to storage error {:?}", e);
                        }
                    }
                },
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::Socket(event)) => {
                    if let Err(e) = vnet_tx.try_send(event) {
                        log::error!("[MediaConnector] forward Sdn SocketEvent error {:?}", e);
                    }
                }
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::RouterSync(event)) => match event {
                    router_sync::Event::DumpRouter(dump) => {
                        let json = serde_json::to_value(dump).expect("should convert json");
                        while let Some(v) = wait_dump_router.pop() {
                            let _ = v.send(json.clone());
                        }
                    }
                },
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
