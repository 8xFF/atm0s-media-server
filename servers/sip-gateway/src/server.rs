use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_std::{channel::bounded, stream::StreamExt};
use cluster::{Cluster, ClusterEndpoint};
use futures::{select, FutureExt};
use media_utils::{SystemTimer, Timer};
use transport::{Transport, TransportIncomingEvent, TransportStateEvent};
use transport_sip::{SipServerSocket, SipServerSocketError, SipServerSocketMessage, SipTransport};

use crate::sip_session::SipSession;

async fn fake_hook(from: &str, to: &str) -> Result<(String, String), ()> {
    async_std::task::sleep(Duration::from_secs(1)).await;
    Ok((to.to_string(), from.to_string()))
}

pub async fn start_server<C, CR>(mut cluster: C, sip_addr: SocketAddr)
where
    C: Cluster<CR> + 'static,
    CR: ClusterEndpoint + 'static,
{
    let mut sip_server = SipServerSocket::new(sip_addr).await;
    let (tx, rx) = bounded::<(SipTransport, String, String)>(100);

    async_std::task::spawn(async move {
        while let Ok((transport, room_id, peer_id)) = rx.recv().await {
            log::info!("[MediaServer] on rtmp connection from {} {}", room_id, peer_id);
            let mut session = match SipSession::new(&room_id, &peer_id, &mut cluster, transport).await {
                Ok(res) => res,
                Err(e) => {
                    log::error!("Error on create rtmp session: {:?}", e);
                    return;
                }
            };

            async_std::task::spawn(async move {
                log::info!("[MediaServer] start loop for rtmp endpoint");
                while let Some(_) = session.recv().await {}
                log::info!("[MediaServer] stop loop for rtmp endpoint");
            });
        }
    });

    let timer = Arc::new(SystemTimer());
    loop {
        match sip_server.recv().await {
            Ok(event) => match event {
                SipServerSocketMessage::RegisterValidate(session, _username) => {
                    //TODO hook this to some kind of auth
                    sip_server.accept_register(session, true);
                }
                SipServerSocketMessage::InCall(socket, req) => {
                    let tx = tx.clone();
                    let timer = timer.clone();
                    async_std::task::spawn(async move {
                        match fake_hook(req.from.uri.user().expect(""), req.to.uri.user().expect("")).await {
                            Ok((room, peer)) => {
                                log::info!("[SipInCall] joined to {room} {peer}");
                                let mut transport = match SipTransport::new(timer.now_ms(), sip_addr, socket, req).await {
                                    Ok(transport) => transport,
                                    Err(e) => {
                                        log::error!("Can not create transport {:?}", e);
                                        return;
                                    }
                                };
                                transport.accept(timer.now_ms());
                                tx.send((transport, room, peer)).await;
                            }
                            Err(_) => {
                                log::info!("[SipInCall] rejected");
                                let mut transport = match SipTransport::new(timer.now_ms(), sip_addr, socket, req).await {
                                    Ok(transport) => transport,
                                    Err(e) => {
                                        log::error!("Can not create transport {:?}", e);
                                        return;
                                    }
                                };
                                transport.reject(timer.now_ms());
                                let mut interval = async_std::stream::interval(Duration::from_millis(50));
                                loop {
                                    select! {
                                        _ = interval.next().fuse() => {
                                            if let Err(e) = transport.on_tick(timer.now_ms()) {
                                                log::error!("Transport error {:?}", e);
                                            }
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
                                log::info!("[SipInCall] ended after rejected");
                            }
                        }
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
