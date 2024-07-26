use std::sync::Arc;

use std::net::SocketAddr;
use tokio::net::UdpSocket;

use crate::ng_controller::commands::{NgRequest, NgResponse};

use super::NgTransport;

pub struct NgUdpTransport {
    socket: Arc<UdpSocket>,
}

impl NgUdpTransport {
    pub async fn new(port: u16) -> Self {
        let socket = UdpSocket::bind(format!("0.0.0.0:{port}")).await.unwrap();
        Self { socket: Arc::new(socket) }
    }
}

impl NgTransport for NgUdpTransport {
    async fn send(&self, res: NgResponse, addr: SocketAddr) {
        let data = res.to_str();
        self.socket.send_to(data.as_bytes(), addr).await.unwrap();
    }

    async fn recv(&self) -> Option<(NgRequest, SocketAddr)> {
        let mut buf = vec![0; 1024];
        match self.socket.recv_from(&mut buf).await {
            Ok((size, addr)) => {
                let data = std::str::from_utf8(&buf[..size]).unwrap();
                if let Some(req) = NgRequest::from_str(data) {
                    Some((req, addr))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}
