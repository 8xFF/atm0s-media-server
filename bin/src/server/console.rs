use std::time::{Duration, Instant};

use atm0s_sdn::{
    features::{neighbours, router_sync, FeaturesControl, FeaturesEvent},
    secure::StaticKeyAuthorization,
    services::{manual2_discovery::AdvertiseTarget, visualization},
    SdnBuilder, SdnControllerUtils, SdnExtOut, SdnOwner, ServiceBroadcastLevel,
};
use clap::Parser;
use media_server_protocol::{
    cluster::{ClusterNodeGenericInfo, ClusterNodeInfo},
    protobuf::cluster_connector::MediaConnectorServiceClient,
    rpc::quinn::QuinnClient,
};
use media_server_secure::jwt::MediaConsoleSecureJwt;
use storage::StorageShared;
use tokio::sync::mpsc::channel;

use crate::{
    http::{run_console_http_server, NodeApiCtx},
    node_metrics::NodeMetricsCollector,
    quinn::{make_quinn_client, VirtualNetwork},
    seeds::refresh_seeds,
    NodeConfig,
};
use sans_io_runtime::backend::PollingBackend;

pub mod socket;
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
    let storage = StorageShared::default();

    let node_id = node.node_id;
    let mut builder = SdnBuilder::<(), SC, SE, TC, TW, ClusterNodeInfo>::new(node_id, &node.bind_addrs, node.bind_addrs_alt);
    let node_addr = builder.node_addr();

    builder.set_authorization(StaticKeyAuthorization::new(&node.secret));
    builder.set_visualization_collector(true);
    builder.set_manual2_discovery(
        vec![
            // for broadcast to other console nodes
            AdvertiseTarget::new(visualization::SERVICE_ID.into(), ServiceBroadcastLevel::Global),
            // for broadcast to all gateway nodes
            AdvertiseTarget::new(media_server_gateway::STORE_SERVICE_ID.into(), ServiceBroadcastLevel::Global),
        ],
        1000,
    );

    let node_info = ClusterNodeInfo::Console(ClusterNodeGenericInfo {
        addr: builder.node_addr().to_string(),
        cpu: 0,
        memory: 0,
        disk: 0,
    });

    let started_at = Instant::now();
    let mut controller = builder.build::<PollingBackend<SdnOwner, 128, 128>>(workers, node_info);
    controller.service_control(visualization::SERVICE_ID.into(), (), visualization::Control::Subscribe.into());
    controller.feature_control((), FeaturesControl::Neighbours(neighbours::Control::Sub));

    let (seed_tx, mut seed_rx) = channel(100);
    refresh_seeds(node_id, &node.seeds, node.seeds_from_url.as_deref(), seed_tx.clone());

    let (mut vnet, vnet_tx, mut vnet_rx) = VirtualNetwork::new(node.node_id);

    let connector_rpc_socket = vnet.udp_socket(0).await.expect("Should open virtual port for gateway rpc");
    let connector_rpc_client = MediaConnectorServiceClient::new(QuinnClient::new(make_quinn_client(connector_rpc_socket, &[]).expect("Should create endpoint for media rpc client")));

    tokio::task::spawn_local(async move { while vnet.recv().await.is_some() {} });

    let (dump_tx, mut dump_rx) = channel(10);
    if let Some(http_port) = http_port {
        let secure = MediaConsoleSecureJwt::from(node.secret.as_bytes());
        let storage = storage.clone();
        let node_ctx = NodeApiCtx { address: node_addr.clone(), dump_tx };
        tokio::spawn(async move {
            if let Err(e) = run_console_http_server(http_port, node_ctx, secure, storage, connector_rpc_client).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    let mut node_metrics_collector = NodeMetricsCollector::default();
    let mut wait_dump_router = vec![];

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

        while let Ok(v) = dump_rx.try_recv() {
            controller.feature_control((), router_sync::Control::DumpRouter.into());
            wait_dump_router.push(v);
        }

        while let Ok(seed) = seed_rx.try_recv() {
            controller.feature_control((), FeaturesControl::Neighbours(neighbours::Control::ConnectTo(seed, true)));
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
                        storage.on_node_removed(started_at.elapsed().as_millis() as u64, node);
                    }
                },
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::Neighbours(event)) => match event {
                    neighbours::Event::Connected(neighbour, conn_id) => {
                        log::info!("Neighbour connected: {:?} {}", neighbour, conn_id);
                    }
                    neighbours::Event::Disconnected(neighbour, conn_id) => {
                        log::info!("Neighbour disconnected: {:?} {}", neighbour, conn_id);
                    }
                    neighbours::Event::SeedAddressNeeded => {
                        log::info!("Seed address needed");
                        refresh_seeds(node_id, &node.seeds, node.seeds_from_url.as_deref(), seed_tx.clone());
                    }
                },
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::Socket(event)) => {
                    if let Err(e) = vnet_tx.try_send(event) {
                        log::error!("forward sdn SocketEvent error {:?}", e);
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
