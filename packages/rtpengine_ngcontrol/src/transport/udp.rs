use std::{net::SocketAddr, str::FromStr};
use tokio::net::UdpSocket;

use crate::commands::{NgRequest, NgResponse};

use super::NgTransport;

pub struct NgUdpTransport {
    socket: UdpSocket,
}

impl NgUdpTransport {
    pub async fn new(addr: SocketAddr) -> Self {
        let socket = UdpSocket::bind(addr).await.expect("Should listen on {port}");
        log::info!("[NgUdpTransport] listen on addr {addr}");
        Self { socket }
    }
}

impl NgTransport for NgUdpTransport {
    async fn send(&self, res: NgResponse, addr: SocketAddr) {
        let data = res.to_str();
        log::info!("[NgUdpTransport] send\n========\n{data}\n==========");
        if let Err(e) = self.socket.send_to(data.as_bytes(), addr).await {
            log::error!("[NgUdpTransport] send response to {addr} error {e:?}");
        }
    }

    async fn recv(&self) -> Option<(NgRequest, SocketAddr)> {
        loop {
            let mut buf = vec![0; 1024];
            match self.socket.recv_from(&mut buf).await {
                Ok((size, addr)) => {
                    log::info!("[NgUdpTransport] recv {size} from {addr}");
                    match std::str::from_utf8(&buf[..size]) {
                        Ok(str) => {
                            log::info!("[NgUdpTransport] recv\n========\n{str}\n==========");
                            if let Ok(req) = NgRequest::from_str(str) {
                                log::info!("[NgUdpTransport] recv req: {req:?}");
                                break Some((req, addr));
                            }
                        }
                        Err(err) => {
                            log::error!("[NgUdpTransport] received invalid utf8 message from {addr}, err {err}");
                        }
                    }
                }
                Err(err) => {
                    log::error!("[NgUdpTransport] udp port error {err}");
                }
            }
        }
    }
}
