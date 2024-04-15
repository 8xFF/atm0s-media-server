use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use poem_openapi::{OpenApi, OpenApiService};

mod webrtc;
mod whep;
mod whip;

#[derive(Default, Clone)]
struct MediaServerCtx {}

struct MediaApis;

#[OpenApi]
impl MediaApis {}

pub async fn run_gateway_http_server() -> Result<(), Box<dyn std::error::Error>> {
    let api_service: OpenApiService<_, ()> = OpenApiService::new(MediaApis, "Media Gateway APIs", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let route = Route::new()
        .nest("/", api_service)
        .nest("/ui", ui)
        .at("/spec", poem::endpoint::make_sync(move |_| spec.clone()))
        .with(Cors::new())
        .data(MediaServerCtx::default());

    Server::new(TcpListener::bind("0.0.0.0:3000")).run(route).await?;
    Ok(())
}

pub async fn run_media_http_server() -> Result<(), Box<dyn std::error::Error>> {
    let api_service: OpenApiService<_, ()> = OpenApiService::new(MediaApis, "Media Server APIs", env!("CARGO_PKG_VERSION")).server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();
    let route = Route::new()
        .nest("/", api_service)
        .nest("/ui", ui)
        .at("/spec", poem::endpoint::make_sync(move |_| spec.clone()))
        .with(Cors::new())
        .data(MediaServerCtx::default());

    Server::new(TcpListener::bind("0.0.0.0:3000")).run(route).await?;
    Ok(())
}
