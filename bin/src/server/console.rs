use std::time::Duration;

use atm0s_sdn::{secure::StaticKeyAuthorization, services::visualization, SdnBuilder, SdnOwner};
use clap::Parser;

use crate::{http::run_console_http_server, NodeConfig};
use sans_io_runtime::backend::PollingBackend;

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SC {
    Visual(visualization::Control),
}

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SE {
    Visual(visualization::Event),
}
type TC = ();
type TW = ();

#[derive(Debug, Parser)]
pub struct Args {}

pub async fn run_console_server(workers: usize, http_port: Option<u16>, node: NodeConfig, _args: Args) {
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    if let Some(http_port) = http_port {
        tokio::spawn(async move {
            if let Err(e) = run_console_http_server(http_port, req_tx).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    let node_id = node.node_id;

    let mut builder = SdnBuilder::<(), SC, SE, TC, TW>::new(node_id, node.udp_port, node.custom_addrs);
    builder.set_authorization(StaticKeyAuthorization::new(&node.secret));

    for seed in node.seeds {
        builder.add_seed(seed);
    }

    let mut controller = builder.build::<PollingBackend<SdnOwner, 128, 128>>(workers);

    loop {
        if controller.process().is_none() {
            break;
        }

        while let Ok(_req) = req_rx.try_recv() {}

        while let Some(_out) = controller.pop_event() {}
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
