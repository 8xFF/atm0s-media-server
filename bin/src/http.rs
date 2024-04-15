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
    req: Req,
    answer_tx: tokio::sync::oneshot::Sender<Res>,
}

impl<Request, Response> Rpc<Request, Response> {
    pub fn new(req: Request) -> (Self, tokio::sync::oneshot::Receiver<Response>) {
        let (answer_tx, answer_rx) = tokio::sync::oneshot::channel();
        (Self { req, answer_tx }, answer_rx)
    }

    pub fn answer(self, res: Response) {
        let _ = self.answer_tx.send(res);
    }
}

mod connector;
mod media;
mod payload_sdp;
mod remote_ip;
mod user_agent;

pub async fn run_gateway_http_server(sender: Sender<media::RpcType>) -> Result<(), Box<dyn std::error::Error>> {
    let api_service: OpenApiService<_, ()> = OpenApiService::new(media::MediaApis, "Media Gateway APIs", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let route = Route::new()
        .nest("/", api_service)
        .nest("/ui", ui)
        .at("/spec", poem::endpoint::make_sync(move |_| spec.clone()))
        .with(Cors::new())
        .data(media::MediaServerCtx { sender });

    Server::new(TcpListener::bind("0.0.0.0:3000")).run(route).await?;
    Ok(())
}

pub async fn run_media_http_server(sender: Sender<media::RpcType>) -> Result<(), Box<dyn std::error::Error>> {
    let api_service: OpenApiService<_, ()> = OpenApiService::new(media::MediaApis, "Media Server APIs", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let route = Route::new()
        .nest("/", api_service)
        .nest("/ui", ui)
        .at("/spec", poem::endpoint::make_sync(move |_| spec.clone()))
        .with(Cors::new())
        .data(media::MediaServerCtx { sender });

    Server::new(TcpListener::bind("0.0.0.0:3000")).run(route).await?;
    Ok(())
}
