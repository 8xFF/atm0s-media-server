use std::net::SocketAddr;
use std::sync::Arc;

use media_server_protocol::endpoint::ClusterConnId;
use media_server_protocol::transport::{RpcReq, RpcRes};
use media_server_secure::{MediaEdgeSecure, MediaGatewaySecure};
use poem::endpoint::StaticFilesEndpoint;
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use poem_openapi::types::{ToJSON, Type};
use poem_openapi::OpenApiService;
use poem_openapi::{types::ParseFromJSON, Object};
use tokio::sync::mpsc::Sender;

mod api_connector;
mod api_media;
mod api_token;
mod utils;

#[derive(Debug, Default, Object)]
pub struct Response<T: ParseFromJSON + ToJSON + Type + Send + Sync> {
    pub status: bool,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[oai(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

pub struct Rpc<Req, Res> {
    pub req: Req,
    pub answer_tx: tokio::sync::oneshot::Sender<Res>,
}

impl<Req, Res> Rpc<Req, Res> {
    pub fn new(req: Req) -> (Self, tokio::sync::oneshot::Receiver<Res>) {
        let (answer_tx, answer_rx) = tokio::sync::oneshot::channel();
        (Self { req, answer_tx }, answer_rx)
    }

    #[allow(unused)]
    pub fn res(self, res: Res) {
        let _ = self.answer_tx.send(res);
    }
}

pub async fn run_gateway_http_server<ES: 'static + MediaEdgeSecure + Send + Sync, GS: 'static + MediaGatewaySecure + Send + Sync>(
    port: u16,
    sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    edge_secure: Arc<ES>,
    gateway_secure: Arc<GS>,
) -> Result<(), Box<dyn std::error::Error>> {
    let token_service: OpenApiService<_, ()> = OpenApiService::new(api_token::TokenApis::<GS>::new(), "App APIs", env!("CARGO_PKG_VERSION")).server("/token/");
    let token_ui = token_service.swagger_ui();
    let token_spec = token_service.spec();
    let media_service: OpenApiService<_, ()> = OpenApiService::new(api_media::MediaApis::<ES>::new(), "Media Gateway APIs", env!("CARGO_PKG_VERSION")).server("/media/");
    let media_ui = media_service.swagger_ui();
    let media_spec = media_service.spec();
    let route = Route::new()
        .nest("/samples", StaticFilesEndpoint::new("./public").index_file("index.html"))
        .nest("/token/", token_service.data(api_token::TokenServerCtx { secure: gateway_secure }))
        .nest("/token/ui", token_ui)
        .at("/token/spec", poem::endpoint::make_sync(move |_| token_spec.clone()))
        .nest("/", media_service.data(api_media::MediaServerCtx { sender, secure: edge_secure }))
        .nest("/ui", media_ui)
        .at("/spec", poem::endpoint::make_sync(move |_| media_spec.clone()))
        .with(Cors::new());

    Server::new(TcpListener::bind(SocketAddr::new([0, 0, 0, 0].into(), port))).run(route).await?;
    Ok(())
}

pub async fn run_media_http_server<ES: 'static + MediaEdgeSecure + Send + Sync, GS: 'static + MediaGatewaySecure + Send + Sync>(
    port: u16,
    sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
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
    let media_service: OpenApiService<_, ()> = OpenApiService::new(api_media::MediaApis::<ES>::new(), "Media Gateway APIs", env!("CARGO_PKG_VERSION")).server("/media/");
    let media_ui = media_service.swagger_ui();
    let media_spec = media_service.spec();
    let route = route
        .nest("/samples", StaticFilesEndpoint::new("./public").index_file("index.html"))
        .nest("/", media_service.data(api_media::MediaServerCtx { sender, secure: edge_secure }))
        .nest("/ui", media_ui)
        .at("/spec", poem::endpoint::make_sync(move |_| media_spec.clone()))
        .with(Cors::new());

    Server::new(TcpListener::bind(SocketAddr::new([0, 0, 0, 0].into(), port))).run(route).await?;
    Ok(())
}
