use std::{collections::HashMap, net::SocketAddr, time::Duration};

use clap::Parser;
use media_server_runner::MediaConfig;
use sans_io_runtime::{backend::PollingBackend, Controller};

use crate::{http::run_media_http_server, server::media::runtime_worker::MediaRuntimeWorker, NodeConfig};

mod runtime_worker;

use runtime_worker::{ExtIn, ExtOut};

#[derive(Debug, Parser)]
pub struct Args {
    custom_addrs: Vec<SocketAddr>,
}

pub async fn run_media_server(workers: usize, node: NodeConfig, args: Args) {
    println!("Running media server");
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    tokio::spawn(async move {
        if let Err(e) = run_media_http_server(req_tx).await {
            log::error!("HTTP Error: {}", e);
        }
    });

    //TODO get local addrs
    let node_id = node.node_id;
    let webrtc_addrs = args.custom_addrs;
    let mut controller = Controller::<_, _, _, _, _, 128>::default();
    for i in 0..workers {
        let cfg = runtime_worker::ICfg {
            controller: i == 0,
            node: node.clone(),
            media: MediaConfig { webrtc_addrs: webrtc_addrs.clone() },
        };
        controller.add_worker::<_, _, MediaRuntimeWorker, PollingBackend<_, 128, 512>>(Duration::from_millis(100), cfg, None);
    }

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

            let (req, _node_id) = req.req.extract();
            let (req, worker) = req.extract();

            let ext = ExtIn::Rpc(req_id, req);
            if let Some(worker) = worker {
                controller.send_to(worker, ext);
            } else {
                controller.send_to_best(ext);
            }
        }

        while let Some(out) = controller.pop_event() {
            match out {
                ExtOut::Rpc(req_id, worker, res) => {
                    let res = res.up_layer(worker).up_layer(node_id);
                    if let Some(tx) = reqs.remove(&req_id) {
                        if let Err(_) = tx.send(res) {
                            log::error!("Send rpc response error for req {req_id}");
                        }
                    }
                }
                ExtOut::Sdn(_) => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
