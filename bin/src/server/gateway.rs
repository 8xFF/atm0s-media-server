use std::{sync::Arc, time::Duration};

use atm0s_sdn::{features::FeaturesEvent, secure::StaticKeyAuthorization, services::visualization, SdnBuilder, SdnControllerUtils, SdnExtOut, SdnOwner};
use clap::Parser;
use media_server_connector::agent_service::ConnectorAgentServiceBuilder;
use media_server_gateway::{store_service::GatewayStoreServiceBuilder, STORE_SERVICE_ID};
use media_server_multi_tenancy::{MultiTenancyStorage, MultiTenancySync};
use media_server_protocol::{
    cluster::{ClusterGatewayInfo, ClusterNodeGenericInfo, ClusterNodeInfo},
    gateway::{generate_gateway_zone_tag, GATEWAY_RPC_PORT},
    protobuf::cluster_gateway::{MediaEdgeServiceClient, MediaEdgeServiceServer},
    rpc::quinn::{QuinnClient, QuinnServer},
};
use media_server_secure::jwt::{MediaEdgeSecureJwt, MediaGatewaySecureJwt};
use rtpengine_ngcontrol::NgUdpTransport;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;

use crate::{
    http::run_gateway_http_server,
    ng_controller::NgControllerServer,
    node_metrics::NodeMetricsCollector,
    quinn::{make_quinn_client, make_quinn_server, VirtualNetwork},
    NodeConfig,
};
use sans_io_runtime::{backend::PollingBackend, ErrorDebugger2};

use self::{dest_selector::build_dest_selector, ip_location::Ip2Location, local_rpc_handler::MediaLocalRpcHandler};

mod dest_selector;
mod ip_location;
mod local_rpc_handler;
mod remote_rpc_handler;

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SC {
    Visual(visualization::Control<ClusterNodeInfo>),
    Gateway(media_server_gateway::store_service::Control),
    Connector(media_server_connector::agent_service::Control),
}

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SE {
    Visual(visualization::Event<ClusterNodeInfo>),
    Gateway(media_server_gateway::store_service::Event),
    Connector(media_server_connector::agent_service::Event),
}
type TC = ();
type TW = ();

#[derive(Debug, Parser)]
pub struct Args {
    /// Location latitude.
    #[arg(env, long, default_value_t = 0.0)]
    lat: f32,

    /// Location longitude.
    #[arg(env, long, default_value_t = 0.0)]
    lon: f32,

    /// Path to the GeoIP database.
    #[arg(env, long, default_value = "./maxminddb-data/GeoLite2-City.mmdb")]
    geo_db: String,

    /// Maximum CPU usage (in percent) allowed for routing to a media node or gateway node.
    #[arg(env, long, default_value_t = 60)]
    max_cpu: u8,

    /// Maximum memory usage (in percent) allowed for routing to a media node or gateway node.
    #[arg(env, long, default_value_t = 80)]
    max_memory: u8,

    /// Maximum disk usage (in percent) allowed for routing to a media node or gateway node.
    #[arg(env, long, default_value_t = 90)]
    max_disk: u8,

    /// The port for binding the RTPengine command UDP socket.
    #[arg(env, long)]
    rtpengine_cmd_addr: Option<SocketAddr>,

    /// multi-tenancy sync endpoint
    #[arg(env, long)]
    multi_tenancy_sync: Option<String>,

    /// multi-tenancy sync endpoint
    #[arg(env, long, default_value_t = 30_000)]
    multi_tenancy_sync_interval_ms: u64,
}

pub async fn run_media_gateway(workers: usize, http_port: Option<u16>, node: NodeConfig, args: Args) {
    rustls::crypto::ring::default_provider().install_default().expect("should install ring as default");

    let default_cluster_cert_buf = include_bytes!("../../certs/cluster.cert");
    let default_cluster_key_buf = include_bytes!("../../certs/cluster.key");
    let default_cluster_cert = CertificateDer::from(default_cluster_cert_buf.to_vec());
    let default_cluster_key = PrivatePkcs8KeyDer::from(default_cluster_key_buf.to_vec());

    // This tx and rx is for sending event to connector in other tasks
    let (connector_agent_tx, mut connector_agent_rx) = tokio::sync::mpsc::channel::<media_server_connector::agent_service::Control>(1024);

    let edge_secure = Arc::new(MediaEdgeSecureJwt::from(node.secret.as_bytes()));

    let app_storage = Arc::new(MultiTenancyStorage::new(&node.secret, None));
    let gateway_secure = MediaGatewaySecureJwt::new(node.secret.as_bytes(), app_storage.clone());
    if let Some(url) = args.multi_tenancy_sync {
        let mut app_sync = MultiTenancySync::new(app_storage, &url, Duration::from_millis(args.multi_tenancy_sync_interval_ms));
        tokio::spawn(async move {
            app_sync.run_loop().await;
        });
    }
    let gateway_secure = Arc::new(gateway_secure);

    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    if let Some(http_port) = http_port {
        let req_tx = req_tx.clone();
        let secure2 = edge_secure.clone();
        tokio::spawn(async move {
            if let Err(e) = run_gateway_http_server(http_port, req_tx, secure2, gateway_secure).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    //Running ng controller for Voip
    if let Some(ngproto_addr) = args.rtpengine_cmd_addr {
        let req_tx = req_tx.clone();
        let rtpengine_udp = NgUdpTransport::new(ngproto_addr).await;
        let secure2 = edge_secure.clone();
        tokio::spawn(async move {
            log::info!("[MediaServer] start ng_controller task");
            let mut server = NgControllerServer::new(rtpengine_udp, secure2, req_tx);
            while server.recv().await.is_some() {}
            log::info!("[MediaServer] stop ng_controller task");
        });
    }

    let node_id = node.node_id;

    let mut builder = SdnBuilder::<(), SC, SE, TC, TW, ClusterNodeInfo>::new(node_id, &node.bind_addrs, node.bind_addrs_alt);
    let node_addr = builder.node_addr();
    let node_info = ClusterNodeInfo::Gateway(
        ClusterNodeGenericInfo {
            addr: node_addr.to_string(),
            cpu: 0,
            memory: 0,
            disk: 0,
        },
        ClusterGatewayInfo {
            lat: args.lat,
            lon: args.lon,
            live: 0,
            max: 0,
        },
    );

    builder.set_authorization(StaticKeyAuthorization::new(&node.secret));
    builder.set_manual_discovery(vec!["gateway".to_string(), generate_gateway_zone_tag(node.zone)], vec!["gateway".to_string()]);
    builder.add_service(Arc::new(GatewayStoreServiceBuilder::new(node.zone, args.lat, args.lon, args.max_cpu, args.max_memory, args.max_disk)));
    builder.add_service(Arc::new(ConnectorAgentServiceBuilder::new()));

    for seed in node.seeds {
        builder.add_seed(seed);
    }

    let mut controller = builder.build::<PollingBackend<SdnOwner, 128, 128>>(workers, node_info);
    let (selector, mut requester) = build_dest_selector();

    // Ip location for routing client to closest gateway
    let ip2location = Arc::new(Ip2Location::new(&args.geo_db));

    //
    // Vnet is a virtual udp layer for creating RPC handlers, we separate media server to 2 layer
    // - async for business logic like proxy, logging handling
    // - sync with sans-io style for media data
    //
    let (mut vnet, vnet_tx, mut vnet_rx) = VirtualNetwork::new(node.node_id);

    let media_rpc_socket = vnet.udp_socket(0).await.expect("Should open virtual port for gateway rpc");
    let media_rpc_client = MediaEdgeServiceClient::new(QuinnClient::new(make_quinn_client(media_rpc_socket, &[]).expect("Should create endpoint for media rpc client")));

    let media_rpc_socket = vnet.udp_socket(GATEWAY_RPC_PORT).await.expect("Should open virtual port for gateway rpc");
    let mut media_rpc_server = MediaEdgeServiceServer::new(
        QuinnServer::new(make_quinn_server(media_rpc_socket, default_cluster_key, default_cluster_cert.clone()).expect("Should create endpoint for media rpc server")),
        remote_rpc_handler::Ctx {
            connector_agent_tx: connector_agent_tx.clone(),
            selector: selector.clone(),
            client: media_rpc_client.clone(),
            ip2location: ip2location.clone(),
        },
        remote_rpc_handler::MediaRemoteRpcHandlerImpl::default(),
    );

    let local_rpc_processor = Arc::new(MediaLocalRpcHandler::new(connector_agent_tx.clone(), selector, media_rpc_client, ip2location));

    tokio::task::spawn_local(async move {
        media_rpc_server.run().await;
    });

    tokio::task::spawn_local(async move { while vnet.recv().await.is_some() {} });

    // Collect node metrics for update to gateway agent service, this information is used inside gateway
    // for forwarding from other gateway
    let mut node_metrics_collector = NodeMetricsCollector::default();
    let mut live_sessions = 0;
    let mut max_sessions = 0;

    // Subscribe ConnectorHandler service
    controller.service_control(media_server_connector::AGENT_SERVICE_ID.into(), (), media_server_connector::agent_service::Control::Sub.into());

    loop {
        if controller.process().is_none() {
            break;
        }

        // Pop from metric collector and pass to Gateway store service
        if let Some(metrics) = node_metrics_collector.pop_measure() {
            let node_info = ClusterNodeInfo::Gateway(
                ClusterNodeGenericInfo {
                    addr: node_addr.to_string(),
                    cpu: metrics.cpu,
                    memory: metrics.memory,
                    disk: metrics.disk,
                },
                ClusterGatewayInfo {
                    lat: args.lat,
                    lon: args.lon,
                    live: live_sessions,
                    max: max_sessions,
                },
            );
            controller.service_control(STORE_SERVICE_ID.into(), (), media_server_gateway::store_service::Control::NodeStats(metrics).into());
            controller.service_control(visualization::SERVICE_ID.into(), (), visualization::Control::UpdateInfo(node_info).into());
        }
        while let Ok(control) = vnet_rx.try_recv() {
            controller.feature_control((), control.into());
        }
        while let Some(out) = requester.recv() {
            controller.service_control(STORE_SERVICE_ID.into(), (), out.into());
        }
        while let Ok(req) = req_rx.try_recv() {
            let res_tx = req.answer_tx;
            let param = req.req;
            let conn_part = param.get_conn_part();
            let local_rpc_processor = local_rpc_processor.clone();

            tokio::spawn(async move {
                let res = local_rpc_processor.process_req(conn_part, param).await;
                res_tx.send(res).print_err2("answer http request error");
            });
        }
        while let Ok(control) = connector_agent_rx.try_recv() {
            controller.service_control(media_server_connector::AGENT_SERVICE_ID.into(), (), control.into());
        }

        while let Some(out) = controller.pop_event() {
            match out {
                SdnExtOut::ServicesEvent(_, _, SE::Gateway(event)) => match event {
                    media_server_gateway::store_service::Event::MediaStats(live, max) => {
                        live_sessions = live;
                        max_sessions = max;
                    }
                    media_server_gateway::store_service::Event::FindNodeRes(req_id, res) => requester.on_find_node_res(req_id, res),
                    media_server_gateway::store_service::Event::FindDestRes(req_id, res) => requester.on_find_dest_res(req_id, res),
                },
                SdnExtOut::ServicesEvent(_, _, SE::Connector(event)) => match event {
                    media_server_connector::agent_service::Event::Stats { queue: _, inflight: _, acked: _ } => {}
                    media_server_connector::agent_service::Event::Response(_) => {}
                },
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::Socket(event)) => {
                    if let Err(e) = vnet_tx.try_send(event) {
                        log::error!("[MediaEdge] forward Sdn SocketEvent error {:?}", e);
                    }
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
