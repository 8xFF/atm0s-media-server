use async_std::channel::{bounded, Receiver, Sender};
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};

mod payload_sdp;
mod rpc_req;

pub use payload_sdp::{ApplicationSdp, HttpResponse};
pub use rpc_req::RpcReqResHttp;

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

    pub async fn start(&mut self, api_service: Route) {
        let route = Route::new().nest("/", api_service).with(Cors::new()).data(self.tx.clone());
        let socket = TcpListener::bind(format!("0.0.0.0:{}", self.port));

        log::info!("Listening http server on 0.0.0.0:{}", self.port);
        async_std::task::spawn(async move {
            Server::new(socket).run(route).await.expect("Should run");
        });
    }

    pub async fn recv(&mut self) -> Option<R> {
        self.rx.recv().await.ok()
    }
}
