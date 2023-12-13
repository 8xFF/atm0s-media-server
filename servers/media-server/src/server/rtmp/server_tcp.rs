use std::net::SocketAddr;

use async_std::channel::{bounded, Receiver};
use media_utils::ErrorDebugger;
use transport::{Transport, TransportIncomingEvent, TransportStateEvent};
use transport_rtmp::RtmpTransport;

pub struct RtmpServer {
    rx: Receiver<(String, String, RtmpTransport)>,
}

impl RtmpServer {
    pub async fn new(port: u16) -> Self {
        let addr = format!("0.0.0.0:{}", port).parse::<SocketAddr>().expect("Should parse ip address");
        let tcp_server = async_std::net::TcpListener::bind(addr).await.expect("Should bind tcp server");
        let (tx, rx) = bounded(1);

        async_std::task::spawn(async move {
            log::info!("Start rtmp server on {}", port);
            while let Ok((stream, addr)) = tcp_server.accept().await {
                log::info!("on rtmp connection from {}", addr);

                let tx = tx.clone();
                async_std::task::spawn_local(async move {
                    let mut transport = RtmpTransport::new(stream);
                    //wait connected or disconnected
                    let mut connected = false;
                    while let Ok(e) = transport.recv(0).await {
                        match e {
                            TransportIncomingEvent::State(state) => {
                                log::info!("[RtmpServer] state: {:?}", state);
                                match state {
                                    TransportStateEvent::Connected => {
                                        connected = true;
                                        break;
                                    }
                                    TransportStateEvent::Disconnected => {
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }

                    if !connected {
                        log::warn!("Rtmp connection not connected");
                        return;
                    }

                    match (transport.room(), transport.peer()) {
                        (Some(r), Some(p)) => {
                            tx.send((r, p, transport)).await.log_error("need send");
                        }
                        _ => {}
                    }
                });
            }
        });

        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<(String, String, RtmpTransport)> {
        self.rx.recv().await.ok()
    }
}
