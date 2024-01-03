use async_std::channel::{bounded, Receiver, Sender};
use metrics_dashboard::HttpMetricMiddleware;
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};

mod authorization;
mod embeded_endpoint;
mod payload_sdp;
mod remote_ip_addr;
mod rpc_req;
mod user_agent;

pub use authorization::TokenAuthorization;
pub use embeded_endpoint::EmbeddedFilesEndpoint;
pub use payload_sdp::{ApplicationSdp, HttpResponse};
pub use remote_ip_addr::RemoteIpAddr;
pub use rpc_req::RpcReqResHttp;
pub use user_agent::UserAgent;

pub struct HttpRpcServer<R: Send> {
    port: u16,
    tx: Sender<R>,
    rx: Receiver<R>,
}

impl<R: 'static + Send> HttpRpcServer<R> {
    pub fn new(port: u16) -> Self {
        let (tx, rx) = bounded(100);
        Self { port, tx, rx }
    }

    pub async fn start<CTX: Send + Sync + Clone + 'static>(&mut self, api_service: Route, ctx: CTX) {
        let route = Route::new().nest("/", api_service).with(Cors::new()).data((self.tx.clone(), ctx));
        let socket = TcpListener::bind(format!("0.0.0.0:{}", self.port));

        log::info!("Listening http server on 0.0.0.0:{}", self.port);
        async_std::task::spawn(async move {
            Server::new(socket).run(route.with(HttpMetricMiddleware)).await.expect("Should run");
        });
    }

    pub async fn recv(&mut self) -> Option<R> {
        self.rx.recv().await.ok()
    }
}
