use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use async_std::{channel::bounded, prelude::FutureExt as _, stream::StreamExt};
use cluster::{
    rpc::{
        gateway::NodeHealthcheckResponse,
        general::MediaEndpointCloseResponse,
        sip::{SipIncomingInviteRequest, SipIncomingInviteStrategy, SipIncomingRegisterRequest, SipOutgoingInviteResponse},
        RpcEmitter, RpcEndpoint, RpcRequest,
    },
    Cluster, ClusterEndpoint,
};
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{select, FutureExt};
use media_utils::{ErrorDebugger, SystemTimer, Timer};
use rsip::{
    headers::CallId,
    typed::{From, To},
    Auth, Host, HostWithPort, Param, Uri,
};
use transport::{Transport, TransportIncomingEvent, TransportStateEvent};
use transport_sip::{GroupId, SipServerSocket, SipServerSocketError, SipServerSocketMessage, SipTransportIn, SipTransportOut};

use crate::{rpc::http::HttpRpcServer, server::MediaServerContext};

fn random_call_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut res = String::new();
    for _ in 0..16 {
        res.push_str(&format!("{:x}", rng.gen::<u8>()));
    }
    res
}

struct ClientInfo {
    username: String,
    addr: SocketAddr,
}

use super::{
    hooks::HooksSender,
    rpc::{cluster::SipClusterRpc, RpcEvent},
    sip_in_session::SipInSession,
    sip_out_session::SipOutSession,
    InternalControl,
};

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
    In(SipTransportIn, String),
    Out(SipTransportOut, String),
}

enum InternalCmd {
    RegisterResult(String, String, GroupId, bool),
}

pub async fn start_server<C, CR, RPC, REQ, EMITTER>(
    mut cluster: C,
    ctx: MediaServerContext<InternalControl>,
    sip_addr: SocketAddr,
    hook_sender: HooksSender,
    mut http_server: HttpRpcServer<RpcEvent>,
    mut rpc_endpoint: SipClusterRpc<RPC, REQ, EMITTER>,
) where
    C: Cluster<CR> + 'static,
    CR: ClusterEndpoint + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let mut sip_server = SipServerSocket::new(sip_addr).await;
    let (tx, rx) = bounded::<(SipTransport, String, String)>(100);

    let ctx_c = ctx.clone();
    async_std::task::spawn(async move {
        while let Ok((transport, room_id, peer_id)) = rx.recv().await {
            log::info!("[MediaServer] on sip connection from {} {}", room_id, peer_id);
            match transport {
                SipTransport::In(transport, conn_id) => {
                    let (rx, conn_id, old_tx) = ctx_c.create_peer(&room_id, &peer_id, Some(conn_id));
                    let mut session = match SipInSession::new(&room_id, &peer_id, &mut cluster, transport, rx).await {
                        Ok(res) => res,
                        Err(e) => {
                            log::error!("Error on create sip session: {:?}", e);
                            return;
                        }
                    };

                    if let Some(old_tx) = old_tx {
                        let (tx, rx) = async_std::channel::bounded(1);
                        old_tx.send(InternalControl::ForceClose(tx)).await.log_error("Should send");
                        rx.recv().timeout(Duration::from_secs(1)).await.log_error("Should ok");
                    }

                    async_std::task::spawn(async move {
                        log::info!("[MediaServer] start loop for sip endpoint");
                        while let Some(_) = session.recv().await {}
                        log::info!("[MediaServer] stop loop for sip endpoint");
                    });
                }
                SipTransport::Out(transport, conn_id) => {
                    let (rx, conn_id, old_tx) = ctx_c.create_peer(&room_id, &peer_id, Some(conn_id));
                    let mut session = match SipOutSession::new(&room_id, &peer_id, &mut cluster, transport, rx).await {
                        Ok(res) => res,
                        Err(e) => {
                            log::error!("Error on create sip session: {:?}", e);
                            return;
                        }
                    };

                    if let Some(old_tx) = old_tx {
                        let (tx, rx) = async_std::channel::bounded(1);
                        old_tx.send(InternalControl::ForceClose(tx)).await.log_error("Should send");
                        rx.recv().timeout(Duration::from_secs(1)).await.log_error("Should ok");
                    }

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
        let rpc = select! {
            rpc = http_server.recv().fuse() => {
                rpc
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc
            },
            e = sip_server.recv().fuse() => match e {
                Ok(event) => match event {
                    SipServerSocketMessage::RegisterValidate(group_id, digest, nonce, username, realm, hashed_password) => {
                        log::info!("Register validate {} {}", username, hashed_password);
                        let session_id = ctx.generate_conn_id();
                        let hook_sender = hook_sender.clone();
                        let internal_tx = internal_tx.clone();
                        async_std::task::spawn(async move {
                            let res = hook_sender.hook_register(SipIncomingRegisterRequest {
                                username: username.clone(),
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

                            internal_tx.send(InternalCmd::RegisterResult(session_id, username, group_id, accept)).await.log_error("should send");
                        });
                        continue;
                    }
                    SipServerSocketMessage::InCall(socket, req) => {
                        let tx = tx.clone();
                        let timer = timer.clone();
                        let hook_sender = hook_sender.clone();
                        let from_number = req.from.uri.user().expect("").to_string();
                        let to_number = req.to.uri.user().expect("").to_string();
                        let conn_id = ctx.generate_conn_id();
                        let hook_req = SipIncomingInviteRequest {
                            source: socket.ctx().remote_addr.to_string(),
                            username: socket.ctx().username.clone(),
                            from_number: from_number.clone(),
                            to_number: to_number.clone(),
                            conn_id: conn_id.clone(),
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
                                                tx.send((SipTransport::In(transport_in, conn_id), room_id.clone(), from_number)).await.log_error("should send");
                                            }
                                            SipIncomingInviteStrategy::Reject => {
                                                transport_in.reject(timer.now_ms()).log_error("should reject");
                                                run_transport(&mut transport_in, timer).await;
                                            }
                                            SipIncomingInviteStrategy::WaitOtherPeers => {
                                                todo!()
                                            }
                                        }
                                    } else {
                                        transport_in.reject(timer.now_ms()).log_error("should reject");
                                        run_transport(&mut transport_in, timer).await;
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error on hook invite {:?}", e);
                                    transport_in.reject(timer.now_ms()).log_error("should reject");
                                    run_transport(&mut transport_in, timer).await;
                                }
                            }
                        });
                        continue;
                    }
                    SipServerSocketMessage::Continue => {
                        continue;
                    }
                },
                Err(e) => match e {
                    SipServerSocketError::MessageParseError => {
                        log::warn!("Can not parse request");
                        continue;
                    }
                    SipServerSocketError::NetworkError(e) => {
                        log::error!("Network error {:?}", e);
                        return;
                    }
                }
            },
            e = internal_rx.recv().fuse() => match e {
                Ok(event) => match event {
                    InternalCmd::RegisterResult(session_id, username, group_id, result) => {
                        if result {
                            sip_server.accept_register(&group_id);
                            sessions.insert(session_id, ClientInfo {
                                username,
                                addr: group_id.0,
                            });
                        } else {
                            sip_server.reject_register(&group_id);
                        }
                        continue;
                    }
                },
                Err(e) => {
                    log::error!("Internal error {:?}", e);
                    return;
                }
            }
        };
        match rpc {
            Some(event) => match event {
                RpcEvent::NodeHeathcheck(req) => {
                    req.answer(Ok(NodeHealthcheckResponse { success: true }));
                }
                RpcEvent::InviteOutgoingClient(req) => {
                    let dest_session_id = req.param().dest_session_id.clone();
                    if let Some(client_info) = sessions.get(&dest_session_id) {
                        let param = req.param().clone();
                        let call_id: CallId = random_call_id().into();
                        let socket = sip_server.create_call(&call_id, client_info.addr);
                        let to_user = client_info.username.clone();
                        let local_from = From {
                            display_name: None,
                            uri: Uri {
                                scheme: Some(rsip::Scheme::Sip),
                                auth: Some(Auth {
                                    user: param.from_number.clone(),
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
                                    host: Host::Domain(client_info.addr.ip().to_string().into()),
                                    port: Some(client_info.addr.port().into()),
                                },
                                ..Default::default()
                            },
                            params: vec![],
                        };

                        let conn_id = ctx.generate_conn_id();
                        let tx = tx.clone();
                        let timer = timer.clone();
                        async_std::task::spawn(async move {
                            if let Ok(transport) = SipTransportOut::new(timer.now_ms(), sip_addr, call_id.clone().into(), local_from, remote_to, socket).await {
                                req.answer(Ok(SipOutgoingInviteResponse { session_id: conn_id.clone() }));
                                tx.send((SipTransport::Out(transport, conn_id), param.room_id, to_user)).await.log_error("should send");
                            } else {
                                req.answer(Err("INTERNAL_ERROR"));
                            }
                        });
                    } else {
                        req.answer(Err("NOT_FOUND"));
                    }
                }
                RpcEvent::InviteOutgoingServer(req) => {
                    todo!()
                }
                RpcEvent::MediaEndpointClose(req) => {
                    if let Some(old_tx) = ctx.get_conn(&req.param().conn_id) {
                        async_std::task::spawn(async move {
                            let (tx, rx) = async_std::channel::bounded(1);
                            old_tx.send(InternalControl::ForceClose(tx.clone())).await.log_error("need send");
                            if let Ok(e) = rx.recv().timeout(Duration::from_secs(1)).await {
                                let control_res = e.map_err(|_e| "INTERNAL_QUEUE_ERROR");
                                req.answer(control_res.map(|_| MediaEndpointCloseResponse { success: true }));
                            } else {
                                req.answer(Err("REQUEST_TIMEOUT"));
                            }
                        });
                    } else {
                        req.answer(Err("NOT_FOUND"));
                    }
                }
            },
            None => {}
        }
    }
}
