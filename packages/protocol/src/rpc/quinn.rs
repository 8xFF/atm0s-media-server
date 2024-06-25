use std::net::SocketAddr;

use quinn::{crypto::rustls::HandshakeData, Connection, Endpoint, Incoming, RecvStream, SendStream};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
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
            let fx = Self::run_incoming(incoming, tx.clone());
            tokio::spawn(async move {
                if let Err(e) = fx.await {
                    log::error!("Quinn Incoming error {:?}", e);
                }
            });
        }
        Some(())
    }

    async fn run_incoming(incoming: Incoming, tx: Sender<(String, QuinnStream)>) -> Result<(), Box<dyn std::error::Error>> {
        let conn = incoming.await?;
        let handshake = conn
            .handshake_data()
            .ok_or("MISSING_HANDSHAKE_DATA".to_string())?
            .downcast::<HandshakeData>()
            .map_err(|_| "MISSING_HANDSHAKE_DATA".to_string())?;
        let server_name = handshake.server_name.ok_or("MISSING_SERVER_NAME".to_string())?;
        let (send, recv) = conn.accept_bi().await?;
        tx.send((
            server_name,
            QuinnStream {
                conn,
                send,
                recv,
                buf: Vec::with_capacity(1500),
                buf_goal: None,
            },
        ))
        .await
        .map_err(|e| e.into())
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
    async fn connect(&self, dest: SocketAddr, server_name: &str) -> Option<QuinnStream> {
        let conn = self.endpoint.connect(dest, server_name).ok()?.await.ok()?;
        let (send, recv) = conn.open_bi().await.ok()?;
        Some(QuinnStream {
            conn,
            send,
            recv,
            buf: Vec::with_capacity(1500),
            buf_goal: None,
        })
    }
}

pub struct QuinnStream {
    conn: Connection,
    send: SendStream,
    recv: RecvStream,
    buf: Vec<u8>,
    buf_goal: Option<usize>,
}

impl QuinnStream {}

impl RpcStream for QuinnStream {
    async fn read(&mut self) -> Option<Vec<u8>> {
        loop {
            if let Some(buf_goal) = self.buf_goal {
                let max_len = buf_goal - self.buf.len();
                if let Some(chunk) = self.recv.read_chunk(max_len, true).await.ok()? {
                    log::debug!("Got frame part {}/{buf_goal}", chunk.bytes.len());
                    self.buf.extend_from_slice(&chunk.bytes);
                    if self.buf.len() == buf_goal {
                        self.buf_goal = None;
                        let res = std::mem::replace(&mut self.buf, Vec::with_capacity(1500));
                        return Some(res);
                    }
                }
            } else {
                let len = self.recv.read_u32().await.ok()?;
                self.buf_goal = Some(len as usize);
                log::debug!("Got frame len {}", len);
                if len == 0 {
                    return Some(vec![]);
                }
            }
        }
    }

    async fn write(&mut self, buf: &[u8]) -> Option<()> {
        log::debug!("Write frame len {}", buf.len());
        self.send.write_u32(buf.len() as u32).await.ok()?;
        self.send.write_all(buf).await.ok()
    }

    async fn close(&mut self) {
        self.conn.closed().await;
    }
}
