use std::net::{IpAddr, SocketAddr};

use atm0s_media_server::{server, NodeConfig};
use atm0s_sdn::NodeAddr;
use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const MAX_ZONE_ID: u32 = 1u32 << 24;

/// Scalable Media Server solution for WebRTC, RTMP, and SIP.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// HTTP port for incoming requests.
    #[arg(env, long)]
    http_port: Option<u16>,

    /// Enable TLS for HTTP connections (set to the port number for HTTPS).
    #[arg(env, long)]
    http_tls: Option<u16>,

    /// SDN (Software-Defined Networking) port.
    #[arg(env, long, default_value_t = 0)]
    sdn_port: u16,

    /// SDN zone identifier, a 24-bit number representing the zone ID.
    #[arg(env, long, default_value_t = 0)]
    sdn_zone_id: u32,

    /// The 8-bit index of the current node within the SDN zone.
    #[arg(env, long, default_value_t = 0)]
    sdn_zone_idx: u8,

    /// Manually specify the IP address of the node. This disables IP autodetection.
    #[arg(env, long)]
    node_ip: Option<IpAddr>,

    /// Alternative IP addresses for the node, useful for environments like AWS or GCP that are behind NAT.
    #[arg(env, long)]
    node_ip_alt: Vec<IpAddr>,

    /// Enable private IP addresses for the node.
    #[arg(env, long)]
    enable_private_ip: bool,

    /// Enable IPv6 support.
    #[arg(env, long)]
    enable_ipv6: bool,

    /// Cluster secret key used for secure communication between nodes.
    #[arg(env, long, default_value = "insecure")]
    secret: String,

    /// Addresses of neighboring nodes for cluster communication.
    #[arg(env, long)]
    seeds: Vec<NodeAddr>,

    /// Number of worker threads to spawn.
    #[arg(env, long, default_value_t = 1)]
    workers: usize,

    /// Disable Sentry error reporting.
    #[arg(env, long)]
    sentry_disable: bool,

    /// Sentry error reporting endpoint.
    #[arg(env, long, default_value = "https://46f5e9a11d430eb479b516fc12033e78@o4507218956386304.ingest.us.sentry.io/4507739106836480")]
    sentry_endpoint: String,

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

    assert!(args.sdn_zone_id < MAX_ZONE_ID, "sdn_zone_id must < {MAX_ZONE_ID}");

    if !args.sentry_disable {
        let _guard = sentry::init((
            args.sentry_endpoint.as_str(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ));
    }

    let http_port = args.http_port;
    let sdn_port = if args.sdn_port > 0 {
        args.sdn_port
    } else {
        // We get a free port
        let udp_socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("Should get free port");
        udp_socket.local_addr().expect("Should get free port").port()
    };

    let workers = args.workers;

    let bind_addrs = if let Some(ip) = args.node_ip {
        vec![SocketAddr::new(ip, sdn_port)]
    } else {
        local_ip_address::list_afinet_netifas()
            .expect("Should have list interfaces")
            .into_iter()
            .filter(|(_, ip)| {
                let allow = match ip {
                    IpAddr::V4(ipv4) => !ipv4.is_private() || args.enable_private_ip,
                    IpAddr::V6(ipv6) => !ipv6.is_unspecified() && !ipv6.is_multicast() && (!ipv6.is_loopback() || args.enable_private_ip) && args.enable_ipv6,
                };
                allow && std::net::UdpSocket::bind(SocketAddr::new(*ip, sdn_port)).is_ok()
            })
            .map(|(_name, ip)| SocketAddr::new(ip, sdn_port))
            .collect::<Vec<_>>()
    };
    let node = NodeConfig {
        node_id: (args.sdn_zone_id << 8) | args.sdn_zone_idx as u32,
        secret: args.secret,
        seeds: args.seeds,
        bind_addrs,
        zone: args.sdn_zone_id << 8,
        bind_addrs_alt: args.node_ip_alt.into_iter().map(|ip| SocketAddr::new(ip, sdn_port)).collect::<Vec<_>>(),
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
