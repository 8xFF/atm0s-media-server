use std::{sync::Arc, time::Duration};

use futures::{select, FutureExt, StreamExt};
use media_utils::{SystemTimer, Timer};
use transport::{Transport, TransportIncomingEvent, TransportStateEvent};
use transport_sip::{SipServerSocket, SipServerSocketError, SipServerSocketMessage, SipTransport};

#[async_std::main]
async fn main() {
    let timer = Arc::new(SystemTimer());
    env_logger::builder().format_timestamp_millis().init();
    let mut sip_server = SipServerSocket::new().await;
    loop {
        match sip_server.recv().await {
            Ok(event) => match event {
                SipServerSocketMessage::InCall(socket, req) => {
                    let mut transport = SipTransport::new(timer.now_ms(), socket, req).await;
                    let timer = timer.clone();
                    async_std::task::spawn(async move {
                        let mut interval = async_std::stream::interval(Duration::from_millis(50));
                        loop {
                            select! {
                                _ = interval.next().fuse() => {
                                    transport.on_tick(timer.now_ms());
                                },
                                e = transport.recv(timer.now_ms()).fuse() => {
                                    match e {
                                        Ok(e) => match e {
                                            TransportIncomingEvent::State(state) => match state {
                                                TransportStateEvent::Disconnected => {
                                                    break;
                                                }
                                                _ => {}
                                            },
                                            _ => {}
                                        }
                                        Err(e) => {
                                            log::error!("Transport error {:?}", e);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        log::info!("Finished InCall transport");
                    });
                }
                SipServerSocketMessage::Continue => {}
            },
            Err(e) => match e {
                SipServerSocketError::MessageParseError => {
                    log::warn!("Can not parse request");
                }
                SipServerSocketError::NetworkError(e) => {
                    log::error!("Network error {:?}", e);
                    return;
                }
            },
        }
    }
}
