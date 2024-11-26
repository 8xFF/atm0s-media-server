use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use atm0s_sdn::NodeAddr;
use clap::Parser;
use media_server_connector::HookBodyType;

use crate::NodeConfig;

#[derive(Debug, Parser)]
pub struct Args {
    /// The port for console server
    #[arg(env, long, default_value_t = 8080)]
    pub console_port: u16,

    /// The port for gateway server
    #[arg(env, long, default_value_t = 3000)]
    pub gateway_port: u16,

    /// The path to the GeoIP database
    #[arg(env, long, default_value = "./maxminddb-data/GeoLite2-City.mmdb")]
    pub geo_db: String,

    /// Maximum CPU usage (in percent) allowed for routing to a media node or gateway node.
    #[arg(env, long, default_value_t = 60)]
    pub max_cpu: u8,

    /// Maximum memory usage (in percent) allowed for routing to a media node or gateway node.
    #[arg(env, long, default_value_t = 80)]
    pub max_memory: u8,

    /// Maximum disk usage (in percent) allowed for routing to a media node or gateway node.
    #[arg(env, long, default_value_t = 90)]
    pub max_disk: u8,

    /// Multi-tenancy sync endpoint
    #[arg(env, long)]
    pub multi_tenancy_sync: Option<String>,

    /// Multi-tenancy sync interval in milliseconds
    #[arg(env, long, default_value_t = 30_000)]
    pub multi_tenancy_sync_interval_ms: u64,

    /// Record cache directory
    #[arg(env, long, default_value = "./record_cache/")]
    pub record_cache: String,

    /// Maximum size of the recording cache in bytes
    #[arg(env, long, default_value_t = 100_000_000)]
    pub record_mem_max_size: usize,

    /// Number of workers for uploading recordings
    #[arg(env, long, default_value_t = 5)]
    pub record_upload_worker: usize,

    /// DB Uri
    #[arg(env, long, default_value = "sqlite://connector.db?mode=rwc")]
    pub db_uri: String,

    /// S3 Uri
    #[arg(env, long, default_value = "http://minioadmin:minioadmin@localhost:9000/record/?path_style=true")]
    pub s3_uri: String,

    /// Hook URI
    #[arg(env, long)]
    pub hook_uri: Option<String>,

    /// Number of workers for hook
    #[arg(env, long, default_value_t = 8)]
    pub hook_workers: usize,

    /// Hook body type
    #[arg(env, long, default_value = "protobuf-json")]
    pub hook_body_type: HookBodyType,

    /// Destroy room after no-one online, default is 2 minutes
    #[arg(env, long, default_value_t = 120_000)]
    pub destroy_room_after_ms: u64,

    /// Storage tick interval, default is 1 minute
    #[arg(env, long, default_value_t = 60_000)]
    pub storage_tick_interval_ms: u64,

    /// The IP address for RTPengine RTP listening.
    /// Default: 127.0.0.1
    #[arg(env, long, default_value = "127.0.0.1")]
    pub rtpengine_listen_ip: IpAddr,

    /// Media instance count
    #[arg(env, long, default_value_t = 2)]
    pub media_instance_count: u32,
}

pub async fn run_standalone(workers: usize, node: NodeConfig, args: Args) {
    log::info!("Running standalone server");
    let console_p2p_addr = {
        log::info!("Running console node");
        let zone = node.zone;
        let secret = node.secret.clone();
        let console_port = args.console_port;
        let console_p2p_addr = get_free_socket_addr();
        tokio::task::spawn_local(async move {
            super::run_console_server(
                workers,
                Some(console_port),
                NodeConfig {
                    node_id: 0,
                    secret,
                    seeds: vec![],
                    bind_addrs: vec![console_p2p_addr],
                    zone,
                    bind_addrs_alt: vec![],
                },
                super::console::Args {},
            )
            .await
        });
        console_p2p_addr
    };
    let gateway_p2p_addr = {
        log::info!("Running gateway node");
        let zone = node.zone;
        let secret = node.secret.clone();
        let gateway_port = args.gateway_port;
        let gateway_p2p_addr = get_free_socket_addr();
        let multi_tenancy_sync = args.multi_tenancy_sync.clone();
        let multi_tenancy_sync_interval_ms = args.multi_tenancy_sync_interval_ms;
        let geo_db = args.geo_db.clone();
        let max_cpu = args.max_cpu;
        let max_memory = args.max_memory;
        let max_disk = args.max_disk;
        tokio::task::spawn_local(async move {
            super::run_media_gateway(
                workers,
                Some(gateway_port),
                NodeConfig {
                    node_id: 10,
                    secret,
                    seeds: vec![NodeAddr::from_str(&format!("0@/ip4/{}/udp/{}", console_p2p_addr.ip(), console_p2p_addr.port())).expect("Should parse node addr")],
                    bind_addrs: vec![gateway_p2p_addr],
                    zone,
                    bind_addrs_alt: vec![],
                },
                super::gateway::Args {
                    lat: 0.0,
                    lon: 0.0,
                    geo_db,
                    max_cpu,
                    max_memory,
                    max_disk,
                    rtpengine_cmd_addr: None,
                    multi_tenancy_sync,
                    multi_tenancy_sync_interval_ms,
                },
            )
            .await
        });
        gateway_p2p_addr
    };
    {
        log::info!("Running connector node");
        let connector_p2p_addr = get_free_socket_addr();
        let secret = node.secret.clone();
        let zone = node.zone;
        let db_uri = args.db_uri.clone();
        let s3_uri = args.s3_uri.clone();
        let hook_uri = args.hook_uri.clone();
        let hook_workers = args.hook_workers;
        let hook_body_type = args.hook_body_type;
        let destroy_room_after_ms = args.destroy_room_after_ms;
        let storage_tick_interval_ms = args.storage_tick_interval_ms;
        let multi_tenancy_sync = args.multi_tenancy_sync.clone();
        let multi_tenancy_sync_interval_ms = args.multi_tenancy_sync_interval_ms;
        tokio::task::spawn_local(async move {
            super::run_media_connector(
                workers,
                None,
                NodeConfig {
                    node_id: 30,
                    secret,
                    seeds: vec![NodeAddr::from_str(&format!("10@/ip4/{}/udp/{}", gateway_p2p_addr.ip(), gateway_p2p_addr.port())).expect("Should parse node addr")],
                    bind_addrs: vec![connector_p2p_addr],
                    zone,
                    bind_addrs_alt: vec![],
                },
                super::connector::Args {
                    db_uri,
                    s3_uri,
                    hook_uri,
                    hook_workers,
                    hook_body_type,
                    destroy_room_after_ms,
                    storage_tick_interval_ms,
                    multi_tenancy_sync,
                    multi_tenancy_sync_interval_ms,
                },
            )
            .await
        });
    }
    for i in 0..args.media_instance_count {
        log::info!("Running media node {}", i);
        let media_p2p_addr = get_free_socket_addr();
        let node_id = 20 + i;
        let secret = node.secret.clone();
        let zone = node.zone;
        let record_cache = args.record_cache.clone();
        let record_mem_max_size = args.record_mem_max_size;
        let record_upload_worker = args.record_upload_worker;
        let rtpengine_listen_ip = args.rtpengine_listen_ip;
        tokio::task::spawn_local(async move {
            super::run_media_server(
                workers,
                None,
                NodeConfig {
                    node_id,
                    secret,
                    seeds: vec![NodeAddr::from_str(&format!("10@/ip4/{}/udp/{}", gateway_p2p_addr.ip(), gateway_p2p_addr.port())).expect("Should parse node addr")],
                    bind_addrs: vec![media_p2p_addr],
                    zone,
                    bind_addrs_alt: vec![],
                },
                super::media::Args {
                    enable_token_api: false,
                    ice_lite: false,
                    webrtc_port_seed: 0,
                    rtpengine_listen_ip,
                    ccu_per_core: 200,
                    record_cache,
                    record_mem_max_size,
                    record_upload_worker,
                    disable_gateway_agent: false,
                    disable_connector_agent: false,
                },
            )
            .await
        });
    }
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}

fn get_free_socket_addr() -> SocketAddr {
    let socket = std::net::UdpSocket::bind(("127.0.0.1", 0)).expect("Should get free port");
    socket.local_addr().expect("Should get free port")
}
