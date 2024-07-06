use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::{Duration, Instant},
};

use atm0s_sdn::{features::FeaturesEvent, SdnExtIn, SdnExtOut, TimePivot, TimeTicker};
use clap::Parser;
use media_server_gateway::ServiceKind;
use media_server_protocol::{
    gateway::GATEWAY_RPC_PORT,
    protobuf::{
        cluster_connector::{connector_request, connector_response},
        cluster_gateway::MediaEdgeServiceServer,
    },
    rpc::quinn::QuinnServer,
};
use media_server_record::MediaRecordService;
use media_server_runner::{MediaConfig, UserData, SE};
use media_server_secure::jwt::{MediaEdgeSecureJwt, MediaGatewaySecureJwt};
use media_server_utils::now_ms;
use rand::random;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use sans_io_runtime::{backend::PollingBackend, Controller};

use crate::{
    http::run_media_http_server,
    node_metrics::NodeMetricsCollector,
    quinn::{make_quinn_server, VirtualNetwork},
    server::media::runtime_worker::MediaRuntimeWorker,
    NodeConfig,
};

mod rpc_handler;
mod runtime_worker;

use runtime_worker::{ExtIn, ExtOut};

#[derive(Debug, Parser)]
pub struct Args {
    /// Enable token API or not, which allow generate token
    #[arg(env, long)]
    enable_token_api: bool,

    /// Webrtc Ice Lite
    #[arg(env, long)]
    ice_lite: bool,

    /// Binding port
    #[arg(env, long, default_value_t = 0)]
    media_port: u16,

    /// Allow private ip
    #[arg(env, long, default_value_t = false)]
    allow_private_ip: bool,

    /// Custom binding address for WebRTC UDP
    #[arg(env, long)]
    custom_ips: Vec<IpAddr>,

    /// Max ccu per core
    #[arg(env, long, default_value_t = 200)]
    ccu_per_core: u32,

    /// Record cache
    #[arg(env, long, default_value = "./record_cache/")]
    record_cache: String,

    /// Record memory max size in bytes
    #[arg(env, long, default_value_t = 100_000_000)]
    record_mem_max_size: usize,

    /// Record upload workers
    #[arg(env, long, default_value_t = 5)]
    record_upload_worker: usize,
}

pub async fn run_media_server(workers: usize, http_port: Option<u16>, node: NodeConfig, args: Args) {
    rustls::crypto::ring::default_provider().install_default().expect("should install ring as default");

    let default_cluster_cert_buf = include_bytes!("../../certs/cluster.cert");
    let default_cluster_key_buf = include_bytes!("../../certs/cluster.key");
    let default_cluster_cert = CertificateDer::from(default_cluster_cert_buf.to_vec());
    let default_cluster_key = PrivatePkcs8KeyDer::from(default_cluster_key_buf.to_vec());

    let secure = Arc::new(MediaEdgeSecureJwt::from(node.secret.as_bytes()));
    let secure2 = args.enable_token_api.then(|| Arc::new(MediaGatewaySecureJwt::from(node.secret.as_bytes())));
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    let req_tx2 = req_tx.clone();
    if let Some(http_port) = http_port {
        let secure = secure.clone();
        tokio::spawn(async move {
            if let Err(e) = run_media_http_server(http_port, req_tx2, secure, secure2).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    let node_id = node.node_id;
    let node_session = random();

    let mut webrtc_addrs = args.custom_ips.into_iter().map(|ip| SocketAddr::new(ip, args.media_port)).collect::<Vec<_>>();
    local_ip_address::local_ip().into_iter().for_each(|ip| {
        if let IpAddr::V4(ip) = ip {
            if !ip.is_private() || args.allow_private_ip {
                println!("Detect local ip: {ip}");
                webrtc_addrs.push(SocketAddr::V4(SocketAddrV4::new(ip, 0)));
            }
        }
    });

    println!("Running media server with addrs: {:?}, ice-lite: {}", webrtc_addrs, args.ice_lite);
    let mut controller = Controller::<_, _, _, _, _, 128>::default();
    for i in 0..workers {
        let cfg = runtime_worker::ICfg {
            controller: i == 0,
            node: node.clone(),
            session: node_session,
            media: MediaConfig {
                webrtc_addrs: webrtc_addrs.clone(),
                ice_lite: args.ice_lite,
                secure: secure.clone(),
                max_live: HashMap::from([(ServiceKind::Webrtc, workers as u32 * args.ccu_per_core)]),
            },
        };
        controller.add_worker::<_, _, MediaRuntimeWorker<_>, PollingBackend<_, 128, 512>>(Duration::from_millis(1), cfg, None);
    }

    for seed in node.seeds {
        controller.send_to(0, ExtIn::Sdn(SdnExtIn::ConnectTo(seed)));
    }

    let mut req_id_seed = 0;
    let mut reqs = HashMap::new();

    //
    // Vnet is a virtual udp layer for creating RPC handlers, we separate media server to 2 layer
    // - async for business logic like proxy, logging handling
    // - sync with sans-io style for media data
    //
    let (mut vnet, vnet_tx, mut vnet_rx) = VirtualNetwork::new(node.node_id);
    let media_rpc_socket = vnet.udp_socket(GATEWAY_RPC_PORT).await.expect("Should open virtual port for gateway rpc");
    let mut media_rpc_server = MediaEdgeServiceServer::new(
        QuinnServer::new(make_quinn_server(media_rpc_socket, default_cluster_key, default_cluster_cert).expect("Should create endpoint for media rpc server")),
        rpc_handler::Ctx { req_tx },
        rpc_handler::MediaRpcHandlerImpl::default(),
    );

    tokio::task::spawn_local(async move {
        media_rpc_server.run().await;
    });

    tokio::task::spawn_local(async move { while vnet.recv().await.is_some() {} });

    // Collect node metrics for update to gateway agent service, this information is used inside gateway
    // for routing to best media-node
    let mut node_metrics_collector = NodeMetricsCollector::default();

    // Collect record packets into chunks and upload to service
    let mut record_service = MediaRecordService::new(args.record_upload_worker, &args.record_cache, args.record_mem_max_size);
    let timer = TimePivot::build();
    let mut ticker = TimeTicker::build(1000);

    loop {
        if controller.process().is_none() {
            break;
        }

        if ticker.tick(Instant::now()) {
            record_service.on_tick(timer.timestamp_ms(Instant::now()));
        }

        // Pop from metric collector and pass to Gateway agent service
        if let Some(metrics) = node_metrics_collector.pop_measure() {
            controller.send_to(
                0, //because sdn controller allway is run inside worker 0
                ExtIn::NodeStats(metrics).into(),
            );
        }
        // Pop control and event from record storage
        while let Some(out) = record_service.pop_output() {
            match out {
                media_server_record::Output::Stats(_) => {
                    //TODO
                }
                media_server_record::Output::UploadRequest(upload_id, req) => {
                    controller.send_to_best(ExtIn::Sdn(SdnExtIn::ServicesControl(
                        media_server_connector::AGENT_SERVICE_ID.into(),
                        UserData::Record(upload_id),
                        media_server_connector::agent_service::Control::Request(now_ms(), connector_request::Request::Record(req)).into(),
                    )));
                }
            }
        }

        while let Ok(control) = vnet_rx.try_recv() {
            controller.send_to_best(ExtIn::Sdn(SdnExtIn::FeaturesControl(media_server_runner::UserData::Cluster, control.into())));
        }
        while let Ok(req) = req_rx.try_recv() {
            let req_id = req_id_seed;
            req_id_seed += 1;
            reqs.insert(req_id, req.answer_tx);

            let (req, _node_id) = req.req.down();
            let (req, worker) = req.down();

            let ext = ExtIn::Rpc(req_id, req);
            if let Some(worker) = worker {
                if worker < workers as u16 {
                    log::info!("on req {req_id} dest to worker {worker}");
                    controller.send_to(worker, ext);
                } else {
                    log::info!("on req {req_id} dest to wrong worker {worker} but workers is {workers}");
                }
            } else {
                log::info!("on req {req_id} dest to any worker");
                controller.send_to_best(ext);
            }
        }

        while let Some(out) = controller.pop_event() {
            match out {
                ExtOut::Rpc(req_id, worker, res) => {
                    log::info!("on req {req_id} res from worker {worker}");
                    let res = res.up(worker).up((node_id, node_session));
                    if let Some(tx) = reqs.remove(&req_id) {
                        if tx.send(res).is_err() {
                            log::error!("Send rpc response error for req {req_id}");
                        }
                    }
                }
                ExtOut::Sdn(SdnExtOut::FeaturesEvent(_, FeaturesEvent::Socket(event))) => {
                    if let Err(e) = vnet_tx.try_send(event) {
                        log::error!("[MediaEdge] forward Sdn SocketEvent error {:?}", e);
                    }
                }
                ExtOut::Sdn(SdnExtOut::ServicesEvent(_service, userdata, SE::Connector(event))) => {
                    match event {
                        media_server_connector::agent_service::Event::Response(res) => match (userdata, res) {
                            (UserData::Record(upload_id), connector_response::Response::Record(res)) => {
                                record_service.on_input(timer.timestamp_ms(Instant::now()), media_server_record::Input::UploadResponse(upload_id, res));
                            }
                            _ => {}
                        },
                        media_server_connector::agent_service::Event::Stats { queue, inflight, acked } => {
                            //TODO
                        }
                    }
                }
                ExtOut::Record(session, ts, event) => {
                    record_service.on_input(timer.timestamp_ms(Instant::now()), media_server_record::Input::Event(session, timer.timestamp_ms(ts), event));
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
