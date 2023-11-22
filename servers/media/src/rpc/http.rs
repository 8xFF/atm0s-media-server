use async_std::channel::{bounded, Receiver, Sender};
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use poem_openapi::OpenApiService;

#[cfg(feature = "embed-samples")]
use poem::endpoint::EmbeddedFilesEndpoint;
#[cfg(feature = "embed-samples")]
use rust_embed::RustEmbed;

#[cfg(not(feature = "embed-samples"))]
use poem::endpoint::StaticFilesEndpoint;

use self::apis::HttpApis;

use super::RpcEvent;

mod apis;
mod payload_sdp;

#[cfg(feature = "embed-samples")]
#[derive(RustEmbed)]
#[folder = "public"]
pub struct Files;

pub struct HttpRpcServer {
    port: u16,
    tx: Sender<RpcEvent>,
    rx: Receiver<RpcEvent>,
}

impl HttpRpcServer {
    pub fn new(port: u16) -> Self {
        let (tx, rx) = bounded(100);
        Self { port, tx, rx }
    }

    pub async fn start(&mut self) {
        let api_service = OpenApiService::new(HttpApis, "Webrtc Server", "1.0.0").server("http://localhost:3000");
        let ui = api_service.swagger_ui();
        let spec = api_service.spec();
        #[cfg(feature = "embed-samples")]
        let samples = EmbeddedFilesEndpoint::<Files>::new();
        #[cfg(not(feature = "embed-samples"))]
        let samples = StaticFilesEndpoint::new("./servers/media/public").show_files_listing().index_file("index.html");

        let route = Route::new()
            .nest("/", api_service)
            .nest("/samples/", samples)
            .nest("/ui/", ui)
            .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
            .with(Cors::new())
            .data(self.tx.clone());
        let socket = TcpListener::bind(format!("0.0.0.0:{}", self.port));

        log::info!("Listening http server on 0.0.0.0:{}", self.port);
        async_std::task::spawn(async move {
            Server::new(socket).run(route).await.expect("Should run");
        });
    }

    pub async fn recv(&mut self) -> Option<RpcEvent> {
        self.rx.recv().await.ok()
    }
}
