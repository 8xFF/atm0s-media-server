use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr, SocketAddrV4},
    time::Duration,
};

use atm0s_sdn::SdnExtIn;
use clap::Parser;
use media_server_runner::MediaConfig;
use sans_io_runtime::{backend::PollingBackend, Controller};

use crate::{http::run_media_http_server, server::media::runtime_worker::MediaRuntimeWorker, NodeConfig};

mod runtime_worker;

use runtime_worker::{ExtIn, ExtOut};

#[derive(Debug, Parser)]
pub struct Args {
    /// Binding port
    #[arg(env, long, default_value_t = 0)]
    media_port: u16,

    /// Allow private ip
    #[arg(env, long, default_value_t = false)]
    allow_private_ip: bool,

    /// Custom binding address for WebRTC UDP
    #[arg(env, long)]
    custom_ips: Vec<IpAddr>,
}

pub async fn run_media_server(workers: usize, http_port: Option<u16>, node: NodeConfig, args: Args) {
    println!("Running media server");
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    if let Some(http_port) = http_port {
        tokio::spawn(async move {
            if let Err(e) = run_media_http_server(http_port, req_tx).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    let node_id = node.node_id;
    let node_session = node.session;
    let mut webrtc_addrs = args.custom_ips.into_iter().map(|ip| SocketAddr::new(ip, args.media_port)).collect::<Vec<_>>();
    local_ip_address::local_ip().into_iter().for_each(|ip| {
        if let IpAddr::V4(ip) = ip {
            if !ip.is_private() || args.allow_private_ip {
                println!("Detect local ip: {ip}");
                webrtc_addrs.push(SocketAddr::V4(SocketAddrV4::new(ip, 0)));
            }
        }
    });

    let mut controller = Controller::<_, _, _, _, _, 128>::default();
    for i in 0..workers {
        let cfg = runtime_worker::ICfg {
            controller: i == 0,
            node: node.clone(),
            media: MediaConfig { webrtc_addrs: webrtc_addrs.clone() },
        };
        controller.add_worker::<_, _, MediaRuntimeWorker, PollingBackend<_, 128, 512>>(Duration::from_millis(1), cfg, None);
    }

    for seed in node.seeds {
        controller.send_to(0, ExtIn::Sdn(SdnExtIn::ConnectTo(seed)));
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
