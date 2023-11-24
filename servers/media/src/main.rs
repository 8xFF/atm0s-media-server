use std::net::SocketAddr;

use clap::Parser;

mod rpc;
mod server;

use cluster::{Cluster, ClusterEndpoint};
use cluster_local::ServerLocal;
use cluster_sdn::{NodeAddr, NodeId, ServerAtm0s, ServerAtm0sConfig};
use server::MediaServer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use transport::{Transport, TransportIncomingEvent, TransportStateEvent};
use transport_rtmp::RtmpTransport;

/// Media Server node
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Http port
    #[arg(env, long, default_value_t = 3000)]
    http_port: u16,

    /// Current Node ID
    #[arg(env, long)]
    node_id: Option<NodeId>,

    /// Neighbors
    #[arg(env, long)]
    neighbours: Vec<NodeAddr>,

    /// Rtmp port
    #[arg(env, long)]
    rtmp_port: Option<u16>,
}

#[async_std::main]
async fn main() {
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();

    async fn start_server<C, CR>(cluster: C, http_port: u16, rtmp_port: Option<u16>)
    where
        C: Cluster<CR> + Send + Sync + 'static,
        CR: ClusterEndpoint + Send + Sync + 'static,
    {
        let (tx, rx) = async_std::channel::bounded(10);

        let mut http_server = rpc::http::HttpRpcServer::new(http_port);
        http_server.start().await;
        let tx_c = tx.clone();
        async_std::task::spawn(async move {
            log::info!("Start http server on {}", http_port);
            while let Some(event) = http_server.recv().await {
                tx_c.send(event).await;
            }
        });

        if let Some(rtmp_port) = rtmp_port {
            let tx_c = tx.clone();
            let addr = format!("0.0.0.0:{rtmp_port}").parse::<SocketAddr>().expect("Should parse ip address");
            let tcp_server = async_std::net::TcpListener::bind(addr).await.expect("Should bind tcp server");
            async_std::task::spawn(async move {
                log::info!("Start rtmp server on {}", rtmp_port);
                while let Ok((stream, addr)) = tcp_server.accept().await {
                    log::info!("on rtmp connection from {}", addr);
                    let tx_c = tx_c.clone();
                    async_std::task::spawn_local(async move {
                        let mut transport = RtmpTransport::new(stream);
                        //wait connected or disconnected
                        let mut connected = false;
                        while let Ok(e) = transport.recv(0).await {
                            match e {
                                TransportIncomingEvent::State(state) => {
                                    log::info!("[RtmpServer] state: {:?}", state);
                                    match state {
                                        TransportStateEvent::Connected => {
                                            connected = true;
                                            break;
                                        }
                                        TransportStateEvent::Disconnected => {
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }

                        if !connected {
                            log::warn!("Rtmp connection not connected");
                            return;
                        }

                        match (transport.room(), transport.peer()) {
                            (Some(r), Some(p)) => {
                                tx_c.send(rpc::RpcEvent::RtmpConnect(transport, r, p)).await;
                            }
                            _ => {}
                        }
                    });
                }
            });
        }

        let mut server = MediaServer::<C, CR>::new(cluster);

        while let Ok(event) = rx.recv().await {
            server.on_incoming(event).await;
        }
    }

    match args.node_id {
        Some(node_id) => {
            let cluster = ServerAtm0s::new(node_id, ServerAtm0sConfig { neighbours: args.neighbours }).await;
            start_server(cluster, args.http_port, args.rtmp_port).await;
        }
        None => {
            let cluster = ServerLocal::new();
            start_server(cluster, args.http_port, args.rtmp_port).await;
        }
    }
}
