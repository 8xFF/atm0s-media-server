use std::net::{IpAddr, SocketAddr};

use atm0s_media_server::{server, NodeConfig};
use atm0s_sdn::{NodeAddr, NodeId};
use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Scalable Media Server solution for WebRTC, RTMP, and SIP.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Http port
    #[arg(env, long)]
    http_port: Option<u16>,

    /// Run http with tls or not
    #[arg(env, long)]
    http_tls: Option<u16>,

    /// Sdn port
    #[arg(env, long, default_value_t = 0)]
    sdn_port: u16,

    /// Sdn Zone, which is 32bit number with last 8bit is 0
    #[arg(env, long, default_value_t = 0)]
    sdn_zone: u32,

    /// Current Node ID
    #[arg(env, long, default_value_t = 1)]
    node_id: NodeId,

    /// Setting the single node IP option will disable the autodetect IP addresses logic
    #[arg(env, long)]
    node_ip: Option<IpAddr>,

    /// Some alternative node IPs, which are useful with some cloud providers behind NAT, like AWS or GCP ...
    #[arg(env, long)]
    node_ip_alt: Vec<IpAddr>,

    /// Enable private ip
    #[arg(env, long)]
    enable_private_ip: bool,

    /// Enable ipv6
    #[arg(env, long)]
    enable_ipv6: bool,

    /// Cluster Secret Key
    #[arg(env, long, default_value = "insecure")]
    secret: String,

    /// Neighbors
    #[arg(env, long)]
    seeds: Vec<NodeAddr>,

    /// Workers
    #[arg(env, long, default_value_t = 1)]
    workers: usize,

    #[command(subcommand)]
    server: server::ServerType,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();

    let http_port = args.http_port;
    let workers = args.workers;
    let bind_addrs = if let Some(ip) = args.node_ip {
        vec![SocketAddr::new(ip, args.sdn_port)]
    } else {
        local_ip_address::list_afinet_netifas()
            .expect("Should have list interfaces")
            .into_iter()
            .filter(|(_, ip)| {
                let allow = match ip {
                    IpAddr::V4(ipv4) => !ipv4.is_private() || args.enable_private_ip,
                    IpAddr::V6(ipv6) => !ipv6.is_unspecified() && !ipv6.is_multicast() && (!ipv6.is_loopback() || args.enable_private_ip) && args.enable_ipv6,
                };
                allow && std::net::UdpSocket::bind(SocketAddr::new(*ip, 0)).is_ok()
            })
            .map(|(_name, ip)| SocketAddr::new(ip, args.sdn_port))
            .collect::<Vec<_>>()
    };
    let node = NodeConfig {
        node_id: args.node_id,
        secret: args.secret,
        seeds: args.seeds,
        bind_addrs,
        zone: args.sdn_zone,
        bind_addrs_alt: args.node_ip_alt.into_iter().map(|ip| SocketAddr::new(ip, args.sdn_port)).collect::<Vec<_>>(),
    };

    log::info!("Bind addrs {:?}, bind addrs alt {:?}", node.bind_addrs, node.bind_addrs_alt);

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            match args.server {
                #[cfg(feature = "console")]
                server::ServerType::Console(args) => server::run_console_server(workers, http_port, node, args).await,
                #[cfg(feature = "gateway")]
                server::ServerType::Gateway(args) => server::run_media_gateway(workers, http_port, node, args).await,
                #[cfg(feature = "connector")]
                server::ServerType::Connector(args) => server::run_media_connector(workers, node, args).await,
                #[cfg(feature = "media")]
                server::ServerType::Media(args) => server::run_media_server(workers, http_port, node, args).await,
                #[cfg(feature = "cert_utils")]
                server::ServerType::Cert(args) => {
                    if let Err(e) = server::run_cert_utils(args).await {
                        log::error!("create cert error {:?}", e);
                    }
                }
            }
        })
        .await;
}
