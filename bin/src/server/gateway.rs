use std::{collections::HashMap, sync::Arc, time::Duration};

use atm0s_sdn::{secure::StaticKeyAuthorization, services::visualization, SdnBuilder, SdnOwner};
use clap::Parser;
use media_server_gateway::store_service::GatewayStoreServiceBuilder;
use media_server_protocol::gateway::generate_gateway_zone_tag;
use media_server_secure::jwt::{MediaEdgeSecureJwt, MediaGatewaySecureJwt};

use crate::{http::run_gateway_http_server, NodeConfig};
use sans_io_runtime::backend::PollingBackend;

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SC {
    Visual(visualization::Control),
    Gateway(media_server_gateway::store_service::Control),
}

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SE {
    Visual(visualization::Event),
    Gateway(media_server_gateway::store_service::Event),
}
type TC = ();
type TW = ();

#[derive(Debug, Parser)]
pub struct Args {
    /// Location latude
    #[arg(env, long, default_value_t = 0.0)]
    lat: f32,

    /// Location longtude
    #[arg(env, long, default_value_t = 0.0)]
    lon: f32,
}

pub async fn run_media_gateway(workers: usize, http_port: Option<u16>, node: NodeConfig, args: Args) {
    let edge_secure = Arc::new(MediaEdgeSecureJwt::from(node.secret.as_bytes()));
    let gateway_secure = Arc::new(MediaGatewaySecureJwt::from(node.secret.as_bytes()));
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    if let Some(http_port) = http_port {
        tokio::spawn(async move {
            if let Err(e) = run_gateway_http_server(http_port, req_tx, edge_secure, gateway_secure).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    let node_id = node.node_id;

    let mut builder = SdnBuilder::<(), SC, SE, TC, TW>::new(node_id, node.udp_port, node.custom_addrs);

    builder.set_authorization(StaticKeyAuthorization::new(&node.secret));
    builder.set_manual_discovery(vec!["gateway".to_string(), generate_gateway_zone_tag(node.zone)], vec!["gateway".to_string()]);
    builder.add_service(Arc::new(GatewayStoreServiceBuilder::new(node.zone, args.lat, args.lon)));

    for seed in node.seeds {
        builder.add_seed(seed);
    }

    let mut controller = builder.build::<PollingBackend<SdnOwner, 128, 128>>(workers);

    let mut req_id_seed = 0;
    let mut reqs = HashMap::new();

    loop {
        if controller.process().is_none() {
            break;
        }
        while let Ok(req) = req_rx.try_recv() {
            let req_id = req_id_seed;
            req_id_seed += 1;
            reqs.insert(req_id, req.answer_tx);
            // let (req, _node_id) = req.req.down();
            // let (req, worker) = req.down();
        }

        while let Some(out) = controller.pop_event() {}
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
