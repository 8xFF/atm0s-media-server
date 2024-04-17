use media_server_protocol::endpoint::ClusterConnId;
use media_server_protocol::transport::{RpcReq, RpcRes};
use poem::endpoint::StaticFilesEndpoint;
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use poem_openapi::types::{ToJSON, Type};
use poem_openapi::OpenApiService;
use poem_openapi::{types::ParseFromJSON, Object};
use tokio::sync::mpsc::Sender;

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

    pub fn res(self, res: Res) {
        let _ = self.answer_tx.send(res);
    }
}

mod api_connector;
mod api_media;
mod utils;

pub async fn run_gateway_http_server(sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Result<(), Box<dyn std::error::Error>> {
    let api_service: OpenApiService<_, ()> = OpenApiService::new(api_media::MediaApis, "Media Gateway APIs", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let route = Route::new()
        .nest("/", api_service)
        .nest("/ui", ui)
        .at("/spec", poem::endpoint::make_sync(move |_| spec.clone()))
        .with(Cors::new())
        .data(api_media::MediaServerCtx { sender });

    Server::new(TcpListener::bind("0.0.0.0:3000")).run(route).await?;
    Ok(())
}

pub async fn run_media_http_server(sender: Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>) -> Result<(), Box<dyn std::error::Error>> {
    let api_service: OpenApiService<_, ()> = OpenApiService::new(api_media::MediaApis, "Media Server APIs", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let route = Route::new()
        .nest("/", api_service)
        .nest("/samples", StaticFilesEndpoint::new("./public").index_file("index.html"))
        .nest("/ui", ui)
        .at("/spec", poem::endpoint::make_sync(move |_| spec.clone()))
        .with(Cors::new())
        .data(api_media::MediaServerCtx { sender });

    Server::new(TcpListener::bind("0.0.0.0:3000")).run(route).await?;
    Ok(())
}
