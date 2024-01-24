use std::sync::Arc;

use clap::{Parser, Subcommand};

mod rpc;
mod server;

use cluster::{
    atm0s_sdn::SystemTimer,
    implement::{NodeAddr, NodeId, ServerSdn, ServerSdnConfig},
    CONNECTOR_SERVICE, GLOBAL_GATEWAY_SERVICE, INNER_GATEWAY_SERVICE, MEDIA_SERVER_SERVICE,
};

#[cfg(feature = "connector")]
use server::connector::run_connector_server;
#[cfg(feature = "gateway")]
use server::gateway::run_gateway_server;
#[cfg(feature = "rtmp")]
use server::rtmp::run_rtmp_server;
#[cfg(feature = "webrtc")]
use server::sip::run_sip_server;
#[cfg(feature = "token_generate")]
use server::token_generate::run_token_generate_server;
#[cfg(feature = "webrtc")]
use server::webrtc::run_webrtc_server;

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::server::gateway::GatewayMode;

/// Media Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Http port
    #[arg(env, long, default_value_t = 3000)]
    http_port: u16,

    /// Run http with tls or not
    #[arg(env, long)]
    http_tls: bool,

    /// Sdn port
    #[arg(env, long, default_value_t = 0)]
    sdn_port: u16,

    /// Sdn group
    #[arg(env, long, default_value = "local")]
    sdn_group: String,

    /// Current Node ID
    #[arg(env, long, default_value_t = 1)]
    node_id: NodeId,

    /// Cluster Secret Key
    #[arg(env, long, default_value = "insecure")]
    secret: String,

    /// Neighbors
    #[arg(env, long)]
    seeds: Vec<NodeAddr>,

    #[command(subcommand)]
    server: Servers,
}

#[derive(Debug, Subcommand)]
enum Servers {
    #[cfg(feature = "token_generate")]
    TokenGenerate(server::token_generate::TokenGenerateArgs),
    #[cfg(feature = "gateway")]
    Gateway(server::gateway::GatewayArgs),
    #[cfg(feature = "webrtc")]
    Webrtc(server::webrtc::WebrtcArgs),
    #[cfg(feature = "rtmp")]
    Rtmp(server::rtmp::RtmpArgs),
    #[cfg(feature = "sip")]
    Sip(server::sip::SipArgs),
    #[cfg(feature = "connector")]
    Connector(server::connector::ConnectorArgs),
}

#[async_std::main]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "atm0s_media_server=info");
    }
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();
    let mut config = ServerSdnConfig {
        secret: args.secret.clone(),
        seeds: args.seeds,
        local_tags: vec![],
        connect_tags: vec![],
    };

    match args.server {
        #[cfg(feature = "token_generate")]
        Servers::TokenGenerate(opts) => {
            let token = Arc::new(cluster::implement::jwt_static::JwtStaticToken::new(&args.secret));
            if let Err(e) = run_token_generate_server(args.http_port, args.http_tls, opts, &args.secret, token).await {
                log::error!("[ConnectorServer] error {}", e);
            }
        }
        #[cfg(feature = "gateway")]
        Servers::Gateway(opts) => {
            use server::MediaServerContext;
            match opts.mode {
                GatewayMode::Global => {
                    config.local_tags = vec!["gateway-global".to_string()];
                    config.connect_tags = vec!["gateway-global".to_string()];
                }
                GatewayMode::Inner => {
                    config.local_tags = vec![format!("gateway-inner-{}", args.sdn_group)];
                    config.connect_tags = vec!["gateway-global".to_string(), format!("gateway-inner-{}", args.sdn_group)];
                }
            }

            let token = Arc::new(cluster::implement::jwt_static::JwtStaticToken::new(&args.secret));
            let ctx = MediaServerContext::<()>::new(args.node_id, 0, Arc::new(SystemTimer()), token.clone(), token);
            let rpc_service_id = match opts.mode {
                GatewayMode::Inner => INNER_GATEWAY_SERVICE,
                GatewayMode::Global => GLOBAL_GATEWAY_SERVICE,
            };
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, args.sdn_port, rpc_service_id, config).await;
            if let Err(e) = run_gateway_server(args.http_port, args.http_tls, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[GatewayServer] error {}", e);
            }
        }
        #[cfg(feature = "webrtc")]
        Servers::Webrtc(opts) => {
            use server::MediaServerContext;
            config.local_tags = vec![format!("media-webrtc-{}", args.sdn_group)];
            config.connect_tags = vec![format!("gateway-inner-{}", args.sdn_group)];

            let token = Arc::new(cluster::implement::jwt_static::JwtStaticToken::new(&args.secret));
            let ctx = MediaServerContext::new(args.node_id, opts.max_conn, Arc::new(SystemTimer()), token.clone(), token);
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, args.sdn_port, MEDIA_SERVER_SERVICE, config).await;
            if let Err(e) = run_webrtc_server(args.http_port, args.http_tls, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[WebrtcServer] error {}", e);
            }
        }
        #[cfg(feature = "rtmp")]
        Servers::Rtmp(opts) => {
            use server::MediaServerContext;
            config.local_tags = vec![format!("media-rtmp-{}", args.sdn_group)];
            config.connect_tags = vec![format!("gateway-inner-{}", args.sdn_group)];

            let token = Arc::new(cluster::implement::jwt_static::JwtStaticToken::new(&args.secret));
            let ctx = MediaServerContext::new(args.node_id, opts.max_conn, Arc::new(SystemTimer()), token.clone(), token);
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, args.sdn_port, MEDIA_SERVER_SERVICE, config).await;
            if let Err(e) = run_rtmp_server(args.http_port, args.http_tls, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[RtmpServer] error {}", e);
            }
        }
        #[cfg(feature = "sip")]
        Servers::Sip(opts) => {
            use server::MediaServerContext;
            config.local_tags = vec![format!("media-sip-{}", args.sdn_group)];
            config.connect_tags = vec![format!("gateway-inner-{}", args.sdn_group)];

            let token = Arc::new(cluster::implement::jwt_static::JwtStaticToken::new(&args.secret));
            let ctx = MediaServerContext::new(args.node_id, opts.max_conn, Arc::new(SystemTimer()), token.clone(), token);
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, args.sdn_port, MEDIA_SERVER_SERVICE, config).await;
            if let Err(e) = run_sip_server(args.http_port, args.http_tls, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[RtmpServer] error {}", e);
            }
        }
        #[cfg(feature = "connector")]
        Servers::Connector(opts) => {
            use server::MediaServerContext;
            config.local_tags = vec![format!("connector-{}", args.sdn_group)];
            config.connect_tags = vec![format!("gateway-inner-{}", args.sdn_group)];

            let token = Arc::new(cluster::implement::jwt_static::JwtStaticToken::new(&args.secret));
            let ctx = MediaServerContext::new(args.node_id, opts.max_conn, Arc::new(SystemTimer()), token.clone(), token);
            let (cluster, rpc_endpoint) = ServerSdn::new(args.node_id, args.sdn_port, CONNECTOR_SERVICE, config).await;
            if let Err(e) = run_connector_server(args.http_port, args.http_tls, opts, ctx, cluster, rpc_endpoint).await {
                log::error!("[ConnectorServer] error {}", e);
            }
        }
    }
}
