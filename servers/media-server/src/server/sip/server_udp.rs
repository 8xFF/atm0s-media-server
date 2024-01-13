use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use async_std::{channel::bounded, stream::StreamExt};
use cluster::{
    rpc::sip::{SipIncomingInviteRequest, SipIncomingInviteStrategy, SipIncomingRegisterRequest},
    Cluster, ClusterEndpoint,
};
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{select, FutureExt};
use media_utils::{ErrorDebugger, SystemTimer, Timer};
use transport::{Transport, TransportIncomingEvent, TransportStateEvent};
use transport_sip::{GroupId, SipServerSocket, SipServerSocketError, SipServerSocketMessage, SipTransportIn, SipTransportOut};

use crate::server::MediaServerContext;

use super::{hooks::HooksSender, sip_in_session::SipInSession, sip_out_session::SipOutSession, InternalControl};

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

enum InternalCmd {
    RegisterResult(String, GroupId, bool),
}

pub async fn start_server<C, CR>(mut cluster: C, ctx: MediaServerContext<InternalControl>, sip_addr: SocketAddr, hook_sender: HooksSender)
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
    let (internal_tx, internal_rx) = bounded::<InternalCmd>(100);
    let mut sessions = HashMap::new();

    loop {
        select! {
            e = sip_server.recv().fuse() => match e {
                Ok(event) => match event {
                    SipServerSocketMessage::RegisterValidate(group_id, digest, nonce, username, realm, hashed_password) => {
                        log::info!("Register validate {} {}", username, hashed_password);
                        let session_id = ctx.generate_conn_id();
                        let hook_sender = hook_sender.clone();
                        let internal_tx = internal_tx.clone();
                        async_std::task::spawn(async move {
                            let res = hook_sender.hook_register(SipIncomingRegisterRequest {
                                username,
                                session_id: session_id.clone(),
                                realm,
                            }).await;

                            let accept = match res {
                                Ok(res) => match (res.success, res.ha1) {
                                    (true, Some(ha1)) => {
                                        let hd2 = md5::compute(format!("REGISTER:{}", digest));
                                        let hd2_str = format!("{:x}", hd2);
                                        let response = md5::compute(format!("{}:{}:{}", ha1, nonce, hd2_str));
                                        let response_str = format!("{:x}", response);
                                        log::info!("Register local calculated md5 hash: {}:{}:{} => {} vs {}", ha1, nonce, hd2_str, response_str, hashed_password);
                                        hashed_password.eq(&response_str)
                                    }
                                    _ => {
                                        log::info!("Register validate failed");
                                        false
                                    }
                                },
                                Err(e) => {
                                    log::error!("Error on hook register {:?}", e);
                                    false
                                }
                            };

                            internal_tx.send(InternalCmd::RegisterResult(session_id, group_id, accept)).await.log_error("should send");
                        });
                    }
                    SipServerSocketMessage::InCall(socket, req) => {
                        let tx = tx.clone();
                        let timer = timer.clone();
                        let hook_sender = hook_sender.clone();
                        let from_number = req.from.uri.user().expect("").to_string();
                        let to_number = req.to.uri.user().expect("").to_string();
                        let call_id = req.call_id.to_string();
                        let hook_req = SipIncomingInviteRequest {
                            source: socket.ctx().remote_addr.to_string(),
                            username: socket.ctx().username.clone(),
                            from_number: from_number.clone(),
                            to_number: to_number.clone(),
                            call_id: call_id,
                            node_id: 0, //TODO
                        };

                        let mut transport_in = match SipTransportIn::new(timer.now_ms(), sip_addr, socket, req).await {
                            Ok(transport) => transport,
                            Err(e) => {
                                log::error!("Can not create transport {:?}", e);
                                continue;
                            }
                        };

                        async_std::task::spawn(async move {
                            match hook_sender.hook_invite(hook_req).await {
                                Ok(hook_res) => {
                                    if let Some(room_id) = hook_res.room_id {
                                        match hook_res.strategy {
                                            SipIncomingInviteStrategy::Accept => {
                                                log::info!("[SipInCall] joined to {room_id} {from_number}");
                                                transport_in.accept(timer.now_ms()).log_error("should accept");
                                                tx.send((SipTransport::In(transport_in), room_id.clone(), from_number)).await.log_error("should send");
                                            }
                                            SipIncomingInviteStrategy::Reject => {
                                                transport_in.reject(timer.now_ms()).log_error("should reject");
                                            }
                                            SipIncomingInviteStrategy::WaitOtherPeers => {
                                                todo!()
                                            }
                                        }
                                    } else {
                                        transport_in.reject(timer.now_ms()).log_error("should reject");
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error on hook invite {:?}", e);
                                    transport_in.reject(timer.now_ms()).log_error("should reject");
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
            },
            e = internal_rx.recv().fuse() => match e {
                Ok(event) => match event {
                    InternalCmd::RegisterResult(session_id, group_id, result) => {
                        if result {
                            sip_server.accept_register(&group_id);
                            sessions.insert(session_id, group_id);
                        } else {
                            sip_server.reject_register(&group_id);
                        }
                    }
                },
                Err(e) => {
                    log::error!("Internal error {:?}", e);
                    return;
                }
            }
        }
    }
}
