use std::net::SocketAddr;

use quinn::{crypto::rustls::HandshakeData, Endpoint, Incoming, RecvStream, SendStream};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinHandle,
};

use super::{RpcClient, RpcServer, RpcStream};

pub struct QuinnServer {
    rx: Receiver<(String, QuinnStream)>,
    task: JoinHandle<Option<()>>,
}

impl QuinnServer {
    pub fn new(endpoint: Endpoint) -> Self {
        let (tx, rx) = channel(10);
        let task = tokio::spawn(Self::run_endpoint(endpoint, tx));
        Self { rx, task }
    }

    async fn run_endpoint(endpoint: Endpoint, tx: Sender<(String, QuinnStream)>) -> Option<()> {
        while let Some(incoming) = endpoint.accept().await {
            tokio::spawn(Self::run_incoming(incoming, tx.clone()));
        }
        Some(())
    }

    async fn run_incoming(incoming: Incoming, tx: Sender<(String, QuinnStream)>) -> Option<()> {
        let conn = incoming.await.ok()?;
        let handshake = conn.handshake_data()?.downcast::<HandshakeData>().ok()?;
        let server_name = handshake.server_name?;
        let (send, recv) = conn.accept_bi().await.ok()?;
        tx.send((server_name, QuinnStream { send, recv })).await.ok()
    }
}

impl Drop for QuinnServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl RpcServer<QuinnStream> for QuinnServer {
    async fn accept(&mut self) -> Option<(String, QuinnStream)> {
        self.rx.recv().await
    }
}

#[derive(Clone)]
pub struct QuinnClient {
    endpoint: Endpoint,
}

impl QuinnClient {
    pub fn new(endpoint: Endpoint) -> Self {
        Self { endpoint }
    }
}

impl RpcClient<SocketAddr, QuinnStream> for QuinnClient {
    async fn connect(&mut self, dest: SocketAddr, server_name: &str) -> Option<QuinnStream> {
        let conn = self.endpoint.connect(dest, server_name).ok()?.await.ok()?;
        let (send, recv) = conn.open_bi().await.ok()?;
        Some(QuinnStream { send, recv })
    }
}

struct QuinnStream {
    send: SendStream,
    recv: RecvStream,
}

impl QuinnStream {}

impl RpcStream for QuinnStream {
    async fn read(&mut self) -> Option<Vec<u8>> {
        self.recv.read_to_end(65000).await.ok()
    }

    async fn write(&mut self, buf: &[u8]) -> Option<()> {
        self.send.write_all(buf).await.ok()
    }
}
