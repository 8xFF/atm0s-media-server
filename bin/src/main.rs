use std::net::{IpAddr, SocketAddr};

use atm0s_media_server::{fetch_node_ip_alt_from_cloud, CloudProvider};
use atm0s_media_server::{server, NodeConfig};
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

    /// Auto generate node_id from last 8 bits of local_ip which match prefix
    /// Example: 192.168.1, or 10.10.10.
    #[arg(env, long)]
    sdn_zone_node_id_from_ip_prefix: Option<String>,

    /// Manually specify the IP address of the node. This disables IP autodetection.
    #[arg(env, long)]
    node_ip: Option<IpAddr>,

    /// Alternative IP addresses for the node, useful for environments like AWS or GCP that are behind NAT.
    #[arg(env, long)]
    node_ip_alt: Vec<IpAddr>,

    /// Auto detect node_ip_alt with some common cloud provider metadata.
    #[arg(env, long)]
    node_ip_alt_cloud: Option<CloudProvider>,

    /// Enable private IP addresses for the node.
    #[arg(env, long, default_value_t = true)]
    enable_private_ip: bool,

    /// Enable loopback IP addresses for the node.
    #[arg(env, long)]
    enable_loopback_ip: bool,

    /// Enable ip from interface's name list, default is allow all.
    #[arg(env, long, value_delimiter = ',')]
    enable_interfaces: Option<Vec<String>>,

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
    /// Currently all of nodes expose /api/node/address endpoint, so we can get seeds from there.
    /// Or we can get from console api /api/cluster/seeds?zone_id=xxx&node_type=xxx
    #[arg(env, long)]
    seeds_from_url: Option<String>,

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
    rustls::crypto::ring::default_provider().install_default().expect("should install ring as default");
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

    let mut auto_generated_node_id = None;
    if let Some(ip_prefix) = args.sdn_zone_node_id_from_ip_prefix {
        for (name, ip) in local_ip_address::list_afinet_netifas().expect("Should have list interfaces") {
            if let IpAddr::V4(ipv4) = ip {
                if ipv4.to_string().starts_with(&ip_prefix) {
                    auto_generated_node_id = Some(ipv4.octets()[3]);
                    log::info!("Found ip prefix {ip_prefix} on {name} with ip {ip} => auto generate sdn_zone_node_id with {}", ipv4.octets()[3]);
                    break;
                }
            }
        }
    }

    let mut node_ip_alt_cloud = vec![];
    if let Some(cloud) = args.node_ip_alt_cloud {
        log::info!("Fetch public ip from cloud provider {:?}", cloud);
        let public_ip = fetch_node_ip_alt_from_cloud(cloud).await.expect("should get node ip alt");
        log::info!("Fetched public ip {:?}", public_ip);
        node_ip_alt_cloud.push(public_ip);
    }

    let bind_addrs = if let Some(ip) = args.node_ip {
        vec![SocketAddr::new(ip, sdn_port)]
    } else {
        local_ip_address::list_afinet_netifas()
            .expect("Should have list interfaces")
            .into_iter()
            .filter(|(name, ip)| {
                let allow_ip_type = match ip {
                    IpAddr::V4(ipv4) => {
                        !ipv4.is_unspecified() && !ipv4.is_multicast() && !ipv4.is_link_local() && (!ipv4.is_loopback() || args.enable_loopback_ip) && (!ipv4.is_private() || args.enable_private_ip)
                    }
                    IpAddr::V6(ipv6) => args.enable_ipv6 && !ipv6.is_unspecified() && !ipv6.is_multicast() && (!ipv6.is_loopback() || args.enable_loopback_ip),
                };
                let allow_interface = args.enable_interfaces.as_ref().map(|names| names.iter().any(|i| i.eq(name.as_str()))).unwrap_or(true);
                log::info!("Interface {name} ip {ip} => allow_ip_type {allow_ip_type}, allow_interface {allow_interface}");
                (allow_ip_type && allow_interface) && std::net::UdpSocket::bind(SocketAddr::new(*ip, sdn_port)).is_ok()
            })
            .map(|(_name, ip)| SocketAddr::new(ip, sdn_port))
            .collect::<Vec<_>>()
    };
    let node = NodeConfig {
        node_id: ZoneId(args.sdn_zone_id).to_node_id(auto_generated_node_id.unwrap_or(args.sdn_zone_node_id)),
        secret: args.secret,
        seeds: args.seeds,
        seeds_from_url: args.seeds_from_url,
        bind_addrs,
        zone: ZoneId(args.sdn_zone_id),
        bind_addrs_alt: node_ip_alt_cloud
            .into_iter()
            .chain(args.node_ip_alt.into_iter())
            .map(|ip| SocketAddr::new(ip, sdn_port))
            .collect::<Vec<_>>(),
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
                server::ServerType::Connector(args) => server::run_media_connector(workers, http_port, node, args).await,
                #[cfg(feature = "media")]
                server::ServerType::Media(args) => server::run_media_server(workers, http_port, node, args).await,
                #[cfg(feature = "cert_utils")]
                server::ServerType::Cert(args) => {
                    if let Err(e) = server::run_cert_utils(args).await {
                        log::error!("create cert error {:?}", e);
                    }
                }
                #[cfg(feature = "standalone")]
                server::ServerType::Standalone(args) => server::run_standalone(workers, node, args).await,
            }
        })
        .await;
}
