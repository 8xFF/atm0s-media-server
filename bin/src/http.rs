use std::net::SocketAddr;
use std::sync::Arc;

use media_server_protocol::endpoint::ClusterConnId;
#[cfg(feature = "console")]
use media_server_protocol::protobuf::cluster_connector::MediaConnectorServiceClient;
#[cfg(feature = "console")]
use media_server_protocol::rpc::quinn::{QuinnClient, QuinnStream};
use media_server_protocol::transport::{RpcReq, RpcRes};
use media_server_secure::{MediaEdgeSecure, MediaGatewaySecure};
#[cfg(not(feature = "embed_static"))]
use poem::endpoint::StaticFilesEndpoint;
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use poem_openapi::types::{ToJSON, Type};
use poem_openapi::OpenApiService;
use poem_openapi::{types::ParseFromJSON, Object};
use tokio::sync::mpsc::Sender;
#[cfg(feature = "embed_static")]
use utils::EmbeddedFilesEndpoint;

mod api_console;
mod api_media;
mod api_token;
mod utils;

#[cfg(feature = "embed_static")]
#[derive(rust_embed::RustEmbed)]
#[folder = "public/media"]
pub struct PublicMediaFiles;

#[cfg(feature = "embed_static")]
#[derive(rust_embed::RustEmbed)]
#[folder = "public/console"]
pub struct PublicConsoleFiles;

#[derive(Debug, Default, Object)]
pub struct Pagination {
    pub total: usize,
    pub current: usize,
}

#[derive(Debug, Object)]
pub struct Response<T: ParseFromJSON + ToJSON + Type + Send + Sync> {
    pub status: bool,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
}

impl<T: ParseFromJSON + ToJSON + Type + Send + Sync> Default for Response<T> {
    fn default() -> Self {
        Self {
            status: false,
            error: None,
            data: None,
            pagination: None,
        }
    }
}

#[cfg(feature = "console")]
pub async fn run_console_http_server(
    port: u16,
    secure: media_server_secure::jwt::MediaConsoleSecureJwt,
    storage: crate::server::console_storage::StorageShared,
    connector: MediaConnectorServiceClient<SocketAddr, QuinnClient, QuinnStream>,
) -> Result<(), Box<dyn std::error::Error>> {
    let user_service: OpenApiService<_, ()> = OpenApiService::new(api_console::user::Apis, "Console User APIs", env!("CARGO_PKG_VERSION")).server("/api/user/");
    let user_ui = user_service.swagger_ui();
    let user_spec = user_service.spec();

    let cluster_service: OpenApiService<_, ()> = OpenApiService::new(api_console::cluster::Apis, "Console Cluster APIs", env!("CARGO_PKG_VERSION")).server("/api/cluster/");
    let cluster_ui = cluster_service.swagger_ui();
    let cluster_spec = cluster_service.spec();

    let connector_service: OpenApiService<_, ()> = OpenApiService::new(api_console::connector::Apis, "Console Connector APIs", env!("CARGO_PKG_VERSION")).server("/api/connector/");
    let connector_ui = connector_service.swagger_ui();
    let connector_spec = connector_service.spec();

    let ctx = api_console::ConsoleApisCtx { secure, storage, connector };

    #[cfg(not(feature = "embed_static"))]
    let console_panel = StaticFilesEndpoint::new("./public/console/").index_file("index.html");
    #[cfg(feature = "embed_static")]
    let console_panel = EmbeddedFilesEndpoint::<PublicConsoleFiles>::new();

    let route = Route::new()
        .nest("/", console_panel)
        //user
        .nest("/api/user/", user_service.data(ctx.clone()))
        .nest("/api/user/ui", user_ui)
        .at("/api/user/spec", poem::endpoint::make_sync(move |_| user_spec.clone()))
        //cluster
        .nest("/api/cluster/", cluster_service.data(ctx.clone()))
        .nest("/api/cluster/ui", cluster_ui)
        .at("/api/cluster/spec", poem::endpoint::make_sync(move |_| cluster_spec.clone()))
        //connector
        .nest("/api/connector/", connector_service.data(ctx.clone()))
        .nest("/api/connector/ui", connector_ui)
        .at("/api/connector/spec", poem::endpoint::make_sync(move |_| connector_spec.clone()))
        .with(Cors::new());

    Server::new(TcpListener::bind(SocketAddr::new([0, 0, 0, 0].into(), port))).run(route).await?;
    Ok(())
}

#[cfg(feature = "gateway")]
pub async fn run_gateway_http_server<ES: 'static + MediaEdgeSecure + Send + Sync, GS: 'static + MediaGatewaySecure + Send + Sync>(
    port: u16,
    sender: Sender<crate::rpc::Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    edge_secure: Arc<ES>,
    gateway_secure: Arc<GS>,
) -> Result<(), Box<dyn std::error::Error>> {
    let token_service: OpenApiService<_, ()> = OpenApiService::new(api_token::TokenApis::<GS>::new(), "App APIs", env!("CARGO_PKG_VERSION")).server("/token/");
    let token_ui = token_service.swagger_ui();
    let token_spec = token_service.spec();

    let webrtc_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::WebrtcApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media Webrtc Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/webrtc/");
    let webrtc_ui = webrtc_service.swagger_ui();
    let webrtc_spec = webrtc_service.spec();

    let whip_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::WhipApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media Whip Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/whip/");
    let whip_ui = whip_service.swagger_ui();
    let whip_spec = whip_service.spec();

    let whep_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::WhepApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media Whep Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/whep/");
    let whep_ui = whep_service.swagger_ui();
    let whep_spec = whep_service.spec();

    let rtpengine_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::RtpengineApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media RtpEngine Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/rtpengine/");
    let rtpengine_ui = rtpengine_service.swagger_ui();
    let rtpengine_spec = rtpengine_service.spec();

    #[cfg(not(feature = "embed_static"))]
    let samples = StaticFilesEndpoint::new("./public/media/").index_file("index.html");
    #[cfg(feature = "embed_static")]
    let samples = EmbeddedFilesEndpoint::<PublicMediaFiles>::new();

    let route = Route::new()
        .nest("/samples", samples)
        .nest("/token/", token_service.data(api_token::TokenServerCtx { secure: gateway_secure }))
        .nest("/token/ui", token_ui)
        .at("/token/spec", poem::endpoint::make_sync(move |_| token_spec.clone()))
        .nest("/webrtc/", webrtc_service)
        .nest("/webrtc/ui", webrtc_ui)
        .at("/webrtc/spec", poem::endpoint::make_sync(move |_| webrtc_spec.clone()))
        .nest("/whip/", whip_service)
        .nest("/whip/ui", whip_ui)
        .at("/whip/spec", poem::endpoint::make_sync(move |_| whip_spec.clone()))
        .nest("/whep/", whep_service)
        .nest("/whep/ui", whep_ui)
        .at("/whep/spec", poem::endpoint::make_sync(move |_| whep_spec.clone()))
        .nest("/rtpengine/", rtpengine_service)
        .nest("/rtpengine/ui", rtpengine_ui)
        .at("/rtpengine/spec", poem::endpoint::make_sync(move |_| rtpengine_spec.clone()))
        .with(Cors::new());

    Server::new(TcpListener::bind(SocketAddr::new([0, 0, 0, 0].into(), port))).run(route).await?;
    Ok(())
}

#[cfg(feature = "media")]
pub async fn run_media_http_server<ES: 'static + MediaEdgeSecure + Send + Sync, GS: 'static + MediaGatewaySecure + Send + Sync>(
    port: u16,
    sender: Sender<crate::rpc::Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    edge_secure: Arc<ES>,
    gateway_secure: Option<Arc<GS>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut route = Route::new();

    if let Some(gateway_secure) = gateway_secure {
        let token_service: OpenApiService<_, ()> = OpenApiService::new(api_token::TokenApis::<GS>::new(), "App APIs", env!("CARGO_PKG_VERSION")).server("/token/");
        let token_ui = token_service.swagger_ui();
        let token_spec = token_service.spec();
        route = route
            .nest("/token/", token_service.data(api_token::TokenServerCtx { secure: gateway_secure }))
            .nest("/token/ui", token_ui)
            .at("/token/spec", poem::endpoint::make_sync(move |_| token_spec.clone()));
    }

    let webrtc_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::WebrtcApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media Webrtc Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/webrtc/");
    let webrtc_ui = webrtc_service.swagger_ui();
    let webrtc_spec = webrtc_service.spec();

    let whip_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::WhipApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media Whip Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/whip/");
    let whip_ui = whip_service.swagger_ui();
    let whip_spec = whip_service.spec();

    let whep_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::WhepApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media Whep Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/whep/");
    let whep_ui = whep_service.swagger_ui();
    let whep_spec = whep_service.spec();

    let rtpengine_service: OpenApiService<_, ()> = OpenApiService::new(
        api_media::RtpengineApis::<ES>::new(sender.clone(), edge_secure.clone()),
        "Media RtpEngine Gateway APIs",
        env!("CARGO_PKG_VERSION"),
    )
    .server("/rtpengine/");
    let rtpengine_ui = rtpengine_service.swagger_ui();
    let rtpengine_spec = rtpengine_service.spec();

    #[cfg(not(feature = "embed_static"))]
    let samples = StaticFilesEndpoint::new("./public/media/").index_file("index.html");
    #[cfg(feature = "embed_static")]
    let samples = EmbeddedFilesEndpoint::<PublicMediaFiles>::new();

    let route = route
        .nest("/samples", samples)
        .nest("/webrtc/", webrtc_service)
        .nest("/webrtc/ui", webrtc_ui)
        .at("/webrtc/spec", poem::endpoint::make_sync(move |_| webrtc_spec.clone()))
        .nest("/whip/", whip_service)
        .nest("/whip/ui", whip_ui)
        .at("/whip/spec", poem::endpoint::make_sync(move |_| whip_spec.clone()))
        .nest("/whep/", whep_service)
        .nest("/whep/ui", whep_ui)
        .at("/whep/spec", poem::endpoint::make_sync(move |_| whep_spec.clone()))
        .nest("/rtpengine/", rtpengine_service)
        .nest("/rtpengine/ui", rtpengine_ui)
        .at("/rtpengine/spec", poem::endpoint::make_sync(move |_| rtpengine_spec.clone()))
        .with(Cors::new());

    Server::new(TcpListener::bind(SocketAddr::new([0, 0, 0, 0].into(), port))).run(route).await?;
    Ok(())
}
