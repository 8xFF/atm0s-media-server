use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use atm0s_sdn::{features::FeaturesEvent, generate_node_addr, SdnExtIn, SdnExtOut, TimePivot, TimeTicker};
use clap::Parser;
use media_server_gateway::ServiceKind;
use media_server_multi_tenancy::MultiTenancyStorage;
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
use rtpengine_ngcontrol::NgUdpTransport;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use sans_io_runtime::{backend::PollingBackend, Controller};

use crate::{
    http::{run_media_http_server, NodeApiCtx},
    ng_controller::NgControllerServer,
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
    /// Enables the Token API, which allows token generation.
    #[arg(env, long)]
    enable_token_api: bool,

    /// Enables WebRTC ICE Lite mode.
    #[arg(env, long)]
    ice_lite: bool,

    /// The seed port for binding the WebRTC UDP socket. The port will increment by one for each worker.
    /// Default: 0, which assigns the port randomly.
    /// If set to 20000, each worker will be assigned a unique port: worker0: 20000, worker1: 20001, worker2: 20002, ...
    #[arg(env, long, default_value_t = 0)]
    webrtc_port_seed: u16,

    /// The port for binding the RTPengine command UDP socket.
    #[arg(env, long)]
    rtpengine_cmd_addr: Option<SocketAddr>,

    /// The IP address for RTPengine RTP listening.
    /// Default: 127.0.0.1
    #[arg(env, long, default_value = "127.0.0.1")]
    rtpengine_rtp_ip: IpAddr,

    /// Maximum concurrent connections per CPU core.
    #[arg(env, long, default_value_t = 200)]
    ccu_per_core: u32,

    /// Directory for storing cached recordings.
    #[arg(env, long, default_value = "./record_cache/")]
    record_cache: String,

    /// Maximum size of the recording cache in bytes.
    #[arg(env, long, default_value_t = 100_000_000)]
    record_mem_max_size: usize,

    /// Number of workers for uploading recordings.
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
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    let node_addr = generate_node_addr(node.node_id, &node.bind_addrs, node.bind_addrs_alt.clone());
    if let Some(http_port) = http_port {
        let secure_gateway = args.enable_token_api.then(|| {
            let app_storage = Arc::new(MultiTenancyStorage::new_with_single(&node.secret, None));
            Arc::new(MediaGatewaySecureJwt::new(node.secret.as_bytes(), app_storage))
        });
        let req_tx = req_tx.clone();
        let secure_edge = secure.clone();
        let node_ctx = NodeApiCtx { address: node_addr.to_string() };
        tokio::spawn(async move {
            if let Err(e) = run_media_http_server(http_port, node_ctx, req_tx, secure_edge, secure_gateway).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    //Running ng controller for Voip
    if let Some(ngproto_addr) = args.rtpengine_cmd_addr {
        let req_tx = req_tx.clone();
        let rtpengine_udp = NgUdpTransport::new(ngproto_addr).await;
        let secure = secure.clone();
        tokio::spawn(async move {
            log::info!("[MediaServer] start ng_controller task");
            let mut server = NgControllerServer::new(rtpengine_udp, secure, req_tx);
            while server.recv().await.is_some() {}
            log::info!("[MediaServer] stop ng_controller task");
        });
    }

    let node_id = node.node_id;
    let node_session = random();

    let mut controller = Controller::<_, _, _, _, _, 128>::default();
    for i in 0..workers {
        let webrtc_port = if args.webrtc_port_seed > 0 {
            args.webrtc_port_seed + i as u16
        } else {
            // We get a free port
            let udp_socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("Should get free port");
            udp_socket.local_addr().expect("Should get free port").port()
        };
        let webrtc_addrs = node.bind_addrs.iter().map(|addr| SocketAddr::new(addr.ip(), webrtc_port)).collect::<Vec<_>>();
        let webrtc_addrs_alt = node.bind_addrs_alt.iter().map(|addr| SocketAddr::new(addr.ip(), webrtc_port)).collect::<Vec<_>>();

        println!("Running media server worker {i} with addrs: {:?}, ice-lite: {}", webrtc_addrs, args.ice_lite);

        let cfg = runtime_worker::ICfg {
            controller: i == 0,
            node: node.clone(),
            session: node_session,
            media: MediaConfig {
                webrtc_addrs,
                webrtc_addrs_alt,
                rtpengine_rtp_ip: args.rtpengine_rtp_ip,
                ice_lite: args.ice_lite,
                secure: secure.clone(),
                max_live: HashMap::from([(ServiceKind::Webrtc, workers as u32 * args.ccu_per_core), (ServiceKind::RtpEngine, workers as u32 * args.ccu_per_core)]),
            },
        };
        controller.add_worker::<_, _, MediaRuntimeWorker<_>, PollingBackend<_, 128, 512>>(Duration::from_millis(1), cfg, None);
    }

    for seed in node.seeds {
        controller.send_to(0, ExtIn::Sdn(SdnExtIn::ConnectTo(seed), true));
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
                ExtIn::NodeStats(metrics),
            );
        }
        // Pop control and event from record storage
        while let Some(out) = record_service.pop_output() {
            match out {
                media_server_record::Output::Stats(_) => {
                    //TODO
                }
                media_server_record::Output::UploadRequest(upload_id, req) => {
                    controller.send_to_best(ExtIn::Sdn(
                        SdnExtIn::ServicesControl(
                            media_server_connector::AGENT_SERVICE_ID.into(),
                            UserData::Record(upload_id),
                            media_server_connector::agent_service::Control::Request(now_ms(), connector_request::Request::Record(req)).into(),
                        ),
                        false,
                    ));
                }
            }
        }

        while let Ok(control) = vnet_rx.try_recv() {
            controller.send_to_best(ExtIn::Sdn(SdnExtIn::FeaturesControl(media_server_runner::UserData::Cluster, control.into()), false));
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
                        media_server_connector::agent_service::Event::Response(res) => {
                            if let (UserData::Record(upload_id), connector_response::Response::Record(res)) = (userdata, res) {
                                record_service.on_input(timer.timestamp_ms(Instant::now()), media_server_record::Input::UploadResponse(upload_id, res));
                            }
                        }
                        media_server_connector::agent_service::Event::Stats { queue: _, inflight: _, acked: _ } => {
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
