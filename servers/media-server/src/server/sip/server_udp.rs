use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use async_std::{channel::bounded, stream::StreamExt};
use cluster::{Cluster, ClusterEndpoint};
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{select, FutureExt};
use media_utils::{ErrorDebugger, SystemTimer, Timer};
use rsip::{
    typed::{From, To},
    Auth, Host, HostWithPort, Param, Uri,
};
use transport::{Transport, TransportIncomingEvent, TransportStateEvent};
use transport_sip::{SipServerSocket, SipServerSocketError, SipServerSocketMessage, SipTransportIn, SipTransportOut};

use crate::server::MediaServerContext;

use super::{sip_in_session::SipInSession, sip_out_session::SipOutSession, InternalControl};

type RmIn = EndpointRpcIn;
type RrIn = RemoteTrackRpcIn;
type RlIn = LocalTrackRpcIn;
type RmOut = EndpointRpcOut;
type RrOut = RemoteTrackRpcOut;
type RlOut = LocalTrackRpcOut;

async fn run_transport<T: Transport<(), RmIn, RrIn, RlIn, RmOut, RrOut, RlOut>>(transport: &mut T, timer: Arc<dyn Timer>) {
    let mut interval = async_std::stream::interval(Duration::from_millis(100));
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
}

enum SipTransport {
    In(SipTransportIn),
    Out(SipTransportOut),
}

pub async fn start_server<C, CR>(mut cluster: C, ctx: MediaServerContext<InternalControl>, sip_addr: SocketAddr)
where
    C: Cluster<CR> + 'static,
    CR: ClusterEndpoint + 'static,
{
    let mut sip_server = SipServerSocket::new(sip_addr).await;
    let (tx, rx) = bounded::<(SipTransport, String, String)>(100);

    async_std::task::spawn(async move {
        while let Ok((transport, room_id, peer_id)) = rx.recv().await {
            log::info!("[MediaServer] on sip connection from {} {}", room_id, peer_id);
            match transport {
                SipTransport::In(transport) => {
                    let mut session = match SipInSession::new(&room_id, &peer_id, &mut cluster, transport).await {
                        Ok(res) => res,
                        Err(e) => {
                            log::error!("Error on create sip session: {:?}", e);
                            return;
                        }
                    };

                    async_std::task::spawn(async move {
                        log::info!("[MediaServer] start loop for sip endpoint");
                        while let Some(_) = session.recv().await {}
                        log::info!("[MediaServer] stop loop for sip endpoint");
                    });
                }
                SipTransport::Out(transport) => {
                    let mut session = match SipOutSession::new(&room_id, &peer_id, &mut cluster, transport).await {
                        Ok(res) => res,
                        Err(e) => {
                            log::error!("Error on create sip session: {:?}", e);
                            return;
                        }
                    };

                    async_std::task::spawn(async move {
                        log::info!("[MediaServer] start loop for sip endpoint");
                        while let Some(_) = session.recv().await {}
                        log::info!("[MediaServer] stop loop for sip endpoint");
                    });
                }
            }
        }
    });

    let timer = Arc::new(SystemTimer());
    let mut users = HashMap::new();
    loop {
        match sip_server.recv().await {
            Ok(event) => match event {
                SipServerSocketMessage::RegisterValidate(session, username, hashed_password) => {
                    users.insert(username, session.0);
                    sip_server.accept_register(session, true);
                }
                SipServerSocketMessage::InCall(socket, req) => {
                    let tx = tx.clone();
                    let timer = timer.clone();

                    let from_user = req.from.uri.user().expect("").to_string();
                    let to_user = req.to.uri.user().expect("").to_string();
                    let room_id = format!("{from_user}-{to_user}-{}", timer.now_ms());
                    let mut transport_in = match SipTransportIn::new(timer.now_ms(), sip_addr, socket, req).await {
                        Ok(transport) => transport,
                        Err(e) => {
                            log::error!("Can not create transport {:?}", e);
                            continue;
                        }
                    };

                    let transport_out: Option<SipTransportOut> = match users.get(&to_user) {
                        Some(addr) => {
                            if let Ok(socket) = sip_server.create_call(&room_id, *addr) {
                                let call_id = room_id.clone().into();
                                let local_from = From {
                                    display_name: None,
                                    uri: Uri {
                                        scheme: Some(rsip::Scheme::Sip),
                                        auth: Some(Auth {
                                            user: from_user.clone(),
                                            password: None,
                                        }),
                                        host_with_port: HostWithPort {
                                            host: Host::Domain(sip_addr.ip().to_string().into()),
                                            port: Some(sip_addr.port().into()),
                                        },
                                        ..Default::default()
                                    },
                                    params: vec![Param::Transport(rsip::Transport::Udp), Param::Tag(timer.now_ms().to_string().into())],
                                };
                                let remote_to = To {
                                    display_name: None,
                                    uri: Uri {
                                        scheme: Some(rsip::Scheme::Sip),
                                        auth: Some(Auth {
                                            user: to_user.clone(),
                                            password: None,
                                        }),
                                        host_with_port: HostWithPort {
                                            host: Host::Domain(addr.ip().to_string().into()),
                                            port: Some(addr.port().into()),
                                        },
                                        ..Default::default()
                                    },
                                    params: vec![],
                                };
                                SipTransportOut::new(timer.now_ms(), sip_addr, call_id, local_from, remote_to, socket).await.ok()
                            } else {
                                None
                            }
                        }
                        None => None,
                    };

                    async_std::task::spawn(async move {
                        if let Some(transport_out) = transport_out {
                            log::info!("[SipInCall] joined to {room_id} {from_user}");
                            transport_in.accept(timer.now_ms()).log_error("should accept");
                            tx.send((SipTransport::In(transport_in), room_id.clone(), from_user)).await.log_error("should send");
                            tx.send((SipTransport::Out(transport_out), room_id, to_user)).await.log_error("should send");
                        } else {
                            log::info!("[SipInCall] rejected");
                            transport_in.reject(timer.now_ms()).log_error("should reject");
                            run_transport(&mut transport_in, timer).await;
                            log::info!("[SipInCall] ended after rejected");
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
