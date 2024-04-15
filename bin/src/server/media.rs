use clap::Parser;

use crate::http::run_media_http_server;

#[derive(Debug, Parser)]
pub struct Args {}

pub async fn run_media_server(args: Args) {
    println!("Running media server");
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    tokio::spawn(async move {
        if let Err(e) = run_media_http_server(req_tx).await {
            log::error!("HTTP Error: {}", e);
        }
    });

    while let Some(rpc) = req_rx.recv().await {}
}
