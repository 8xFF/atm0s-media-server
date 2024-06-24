use std::time::{Duration, Instant};

use atm0s_sdn::{features::FeaturesEvent, secure::StaticKeyAuthorization, services::visualization, SdnBuilder, SdnControllerUtils, SdnExtOut, SdnOwner};
use clap::Parser;
use media_server_protocol::{
    cluster::{ClusterNodeGenericInfo, ClusterNodeInfo},
    protobuf::cluster_connector::MediaConnectorServiceClient,
    rpc::quinn::QuinnClient,
};
use media_server_secure::jwt::MediaConsoleSecureJwt;
use storage::StorageShared;

use crate::{
    http::run_console_http_server,
    node_metrics::NodeMetricsCollector,
    quinn::{make_quinn_client, VirtualNetwork},
    NodeConfig,
};
use sans_io_runtime::backend::PollingBackend;

pub mod storage;

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SC {
    Visual(visualization::Control<ClusterNodeInfo>),
}

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SE {
    Visual(visualization::Event<ClusterNodeInfo>),
}
type TC = ();
type TW = ();

#[derive(Debug, Parser)]
pub struct Args {}

pub async fn run_console_server(workers: usize, http_port: Option<u16>, node: NodeConfig, _args: Args) {
    rustls::crypto::ring::default_provider().install_default().expect("should install ring as default");

    let storage = StorageShared::default();

    let node_id = node.node_id;
    let mut builder = SdnBuilder::<(), SC, SE, TC, TW, ClusterNodeInfo>::new(node_id, node.udp_port, node.custom_addrs);
    let node_addr = builder.node_addr();

    builder.set_authorization(StaticKeyAuthorization::new(&node.secret));
    builder.set_visualization_collector(true);

    for seed in node.seeds {
        builder.add_seed(seed);
    }

    let node_info = ClusterNodeInfo::Console(ClusterNodeGenericInfo {
        addr: builder.node_addr().to_string(),
        cpu: 0,
        memory: 0,
        disk: 0,
    });

    let started_at = Instant::now();
    let mut controller = builder.build::<PollingBackend<SdnOwner, 128, 128>>(workers, node_info);
    controller.service_control(visualization::SERVICE_ID.into(), (), visualization::Control::Subscribe.into());

    let (mut vnet, vnet_tx, mut vnet_rx) = VirtualNetwork::new(node.node_id);

    let connector_rpc_socket = vnet.udp_socket(0).await.expect("Should open virtual port for gateway rpc");
    let connector_rpc_client = MediaConnectorServiceClient::new(QuinnClient::new(make_quinn_client(connector_rpc_socket, &[]).expect("Should create endpoint for media rpc client")));

    tokio::task::spawn_local(async move { while vnet.recv().await.is_some() {} });

    if let Some(http_port) = http_port {
        let secure = MediaConsoleSecureJwt::from(node.secret.as_bytes());
        let storage = storage.clone();
        tokio::spawn(async move {
            if let Err(e) = run_console_http_server(http_port, secure, storage, connector_rpc_client).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    let mut node_metrics_collector = NodeMetricsCollector::default();

    loop {
        if controller.process().is_none() {
            break;
        }

        while let Ok(control) = vnet_rx.try_recv() {
            controller.feature_control((), control.into());
        }

        if let Some(metrics) = node_metrics_collector.pop_measure() {
            let node_info = ClusterNodeInfo::Console(ClusterNodeGenericInfo {
                addr: node_addr.to_string(),
                cpu: metrics.cpu,
                memory: metrics.memory,
                disk: metrics.disk,
            });
            controller.service_control(visualization::SERVICE_ID.into(), (), visualization::Control::UpdateInfo(node_info).into());
            storage.on_tick(started_at.elapsed().as_millis() as u64);
        }

        while let Some(out) = controller.pop_event() {
            match out {
                SdnExtOut::ServicesEvent(_service, (), SE::Visual(event)) => match event {
                    visualization::Event::GotAll(all) => {
                        log::info!("Got all: {:?}", all);
                    }
                    visualization::Event::NodeChanged(node, info, conns) => {
                        log::info!("Node set: {:?} {:?} {:?}", node, info, conns);
                        storage.on_ping(started_at.elapsed().as_millis() as u64, node, info, conns);
                    }
                    visualization::Event::NodeRemoved(node) => {
                        log::info!("Node del: {:?}", node);
                    }
                },
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::Socket(event)) => {
                    if let Err(e) = vnet_tx.try_send(event) {
                        log::error!("forward sdn SocketEvent error {:?}", e);
                    }
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
