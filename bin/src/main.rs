use std::net::{IpAddr, SocketAddr};

use atm0s_media_server::{fetch_node_addr_from_api, server, NodeConfig};
use atm0s_sdn::NodeAddr;
use clap::Parser;
use media_server_protocol::cluster::ZoneId;
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
    sdn_zone_node_id: u8,

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

    /// Seeds from API, this is used for auto-discovery of seeds.
    /// It is very useful for cloud deployment.
    /// Currently all of nodes expose /api/node/address endpoint, so we can get seeds from there.
    #[arg(env, long)]
    seeds_from_node_api: Option<String>,

    /// Number of worker threads to spawn.
    #[arg(env, long, default_value_t = 1)]
    workers: usize,

    /// Sentry error reporting endpoint.
    #[arg(env, long)]
    sentry_endpoint: Option<String>,

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

    let _guard = args.sentry_endpoint.map(|sentry_endpoint| {
        sentry::init((
            sentry_endpoint.as_str(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ))
    });

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
    let mut node = NodeConfig {
        node_id: ZoneId(args.sdn_zone_id).to_node_id(args.sdn_zone_node_id),
        secret: args.secret,
        seeds: args.seeds,
        bind_addrs,
        zone: ZoneId(args.sdn_zone_id),
        bind_addrs_alt: args.node_ip_alt.into_iter().map(|ip| SocketAddr::new(ip, sdn_port)).collect::<Vec<_>>(),
    };

    log::info!("Bind addrs {:?}, bind addrs alt {:?}", node.bind_addrs, node.bind_addrs_alt);

    if let Some(seeds_from_node_api) = args.seeds_from_node_api {
        log::info!("Generate seeds from node_api {}", seeds_from_node_api);
        let addr = fetch_node_addr_from_api(&seeds_from_node_api).await.expect("should get seed");
        log::info!("Generated seed {:?}", addr);
        node.seeds = vec![addr];
    }

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
