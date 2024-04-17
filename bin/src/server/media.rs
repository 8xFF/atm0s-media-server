use std::{collections::HashMap, time::Duration};

use clap::Parser;
use sans_io_runtime::{backend::PollingBackend, Controller};

use crate::{http::run_media_http_server, server::media::runtime_worker::MediaRuntimeWorker};

use super::MediaSdnConfig;

mod runtime_worker;

use runtime_worker::{ExtIn, ExtOut};

#[derive(Debug, Parser)]
pub struct Args {}

pub async fn run_media_server(sdn: MediaSdnConfig, args: Args) {
    println!("Running media server");
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    tokio::spawn(async move {
        if let Err(e) = run_media_http_server(req_tx).await {
            log::error!("HTTP Error: {}", e);
        }
    });

    let mut controller = Controller::<_, _, _, _, _, 128>::default();
    controller.add_worker::<_, _, MediaRuntimeWorker, PollingBackend<_, 128, 512>>(Duration::from_millis(100), (), None);

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

            let ext = ExtIn::Rpc(req_id, req.req);
            controller.send_to_best(ext);
        }

        while let Some(out) = controller.pop_event() {
            match out {
                ExtOut::Rpc(req_id, res) => {
                    if let Some(tx) = reqs.remove(&req_id) {
                        if let Err(_) = tx.send(res) {
                            log::error!("Send rpc response error for req {req_id}");
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
