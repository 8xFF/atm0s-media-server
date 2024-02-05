use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use async_std::{
    channel::{bounded, Sender},
    prelude::FutureExt as _,
    stream::StreamExt,
};
use cluster::{
    rpc::{
        general::MediaEndpointCloseResponse,
        sip::{SipIncomingAuthRequest, SipIncomingInviteRequest, SipIncomingInviteStrategy, SipIncomingRegisterRequest, SipIncomingUnregisterRequest, SipOutgoingInviteResponse},
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
use transport_sip::{sip_request::SipRequest, GroupId, SipMessage, SipServerSocket, SipServerSocketError, SipServerSocketMessage, SipTransportIn, SipTransportOut, VirtualSocket};

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

struct ClientSockInfo {
    username: String,
    realm: String,
    ha1_hash: String,
    session_id: String,
    last_ts: u64,
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

async fn run_transport<T: Transport<(), RmIn, RrIn, RlIn, RmOut, RrOut, RlOut>>(transport: &mut T, timer: &Arc<dyn Timer>) {
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
                        break;
                    }
                }
            }
        }
    }
}

fn run_incoming_call(
    sip_addr: SocketAddr,
    hook_sender: HooksSender,
    socket: VirtualSocket<GroupId, SipMessage>,
    req: SipRequest,
    ctx: &MediaServerContext<InternalControl>,
    timer: Arc<dyn Timer>,
    tx: Sender<(SipTransport, String, String)>,
) {
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

    let mut transport_in = match SipTransportIn::new(timer.now_ms(), sip_addr, socket, req) {
        Ok(transport) => transport,
        Err(e) => {
            log::error!("Can not create transport {:?}", e);
            return;
        }
    };

    async_std::task::spawn(async move {
        let hook_res = loop {
            select! {
                _ = run_transport(&mut transport_in, &timer).fuse() => {
                    return;
                },
                e = hook_sender.hook_invite(hook_req).fuse() => break e,
            }
        };
        match hook_res {
            Ok(hook_res) => {
                if let Some(room_id) = hook_res.room_id {
                    match hook_res.strategy {
                        SipIncomingInviteStrategy::Accept => {
                            log::info!("[SipInCall] accept then join to {room_id} {from_number}");
                            transport_in.accept(timer.now_ms()).log_error("should accept");
                            tx.send((SipTransport::In(transport_in, conn_id), room_id.clone(), from_number)).await.log_error("should send");
                        }
                        SipIncomingInviteStrategy::Reject => {
                            transport_in.reject(timer.now_ms()).log_error("should reject");
                            run_transport(&mut transport_in, &timer).await;
                        }
                        SipIncomingInviteStrategy::WaitOtherPeers => {
                            log::info!("[SipInCall] join to {room_id} {from_number}");
                            transport_in.ringing(timer.now_ms()).log_error("should accept");
                            tx.send((SipTransport::In(transport_in, conn_id), room_id.clone(), from_number)).await.log_error("should send");
                        }
                    }
                } else {
                    transport_in.reject(timer.now_ms()).log_error("should reject");
                    run_transport(&mut transport_in, &timer).await;
                }
            }
            Err(e) => {
                log::error!("Error on hook invite {:?}", e);
                transport_in.reject(timer.now_ms()).log_error("should reject");
                run_transport(&mut transport_in, &timer).await;
            }
        }
    });
}

fn run_outgoing_call(
    sip_addr: SocketAddr,
    sip_server: &mut SipServerSocket,
    room_id: String,
    from_number: String,
    to_number: String,
    to_addr: SocketAddr,
    ctx: &MediaServerContext<InternalControl>,
    timer: Arc<dyn Timer>,
    tx: Sender<(SipTransport, String, String)>,
) -> Result<SipOutgoingInviteResponse, &'static str> {
    let call_id: CallId = random_call_id().into();
    let socket = sip_server.create_call(&call_id, to_addr);

    let local_from = From {
        display_name: None,
        uri: Uri {
            scheme: Some(rsip::Scheme::Sip),
            auth: Some(Auth { user: from_number, password: None }),
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
                user: to_number.clone(),
                password: None,
            }),
            host_with_port: HostWithPort {
                host: Host::Domain(to_addr.ip().to_string().into()),
                port: Some(to_addr.port().into()),
            },
            ..Default::default()
        },
        params: vec![],
    };

    let conn_id = ctx.generate_conn_id();
    if let Ok(transport) = SipTransportOut::new(timer.now_ms(), sip_addr, call_id.clone().into(), local_from, remote_to, socket) {
        let session_id = conn_id.clone();
        async_std::task::spawn(async move {
            tx.send((SipTransport::Out(transport, conn_id), room_id, to_number)).await.log_error("should send");
        });

        Ok(SipOutgoingInviteResponse { session_id })
    } else {
        Err("INTERNAL_ERROR")
    }
}

enum SipTransport {
    In(SipTransportIn, String),
    Out(SipTransportOut, String),
}

enum InternalCmd {
    RegisterResult(String, String, String, Option<String>, GroupId, bool),
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
                    let (rx, _conn_id, old_tx) = ctx_c.create_peer(&room_id, &peer_id, Some(conn_id));
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
                    let (rx, _conn_id, old_tx) = ctx_c.create_peer(&room_id, &peer_id, Some(conn_id));
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
    let mut tick = async_std::stream::interval(Duration::from_millis(100));
    let (internal_tx, internal_rx) = bounded::<InternalCmd>(100);
    let mut client_sockets = HashMap::<SocketAddr, ClientSockInfo>::new();
    let mut sessions = HashMap::new();

    loop {
        let rpc = select! {
            rpc = http_server.recv().fuse() => {
                rpc
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc
            },
            _ = tick.next().fuse() => {
                let timeout_clients = client_sockets.iter().filter_map(|(addr, info)| {
                    if timer.now_ms() - info.last_ts > 180000 {
                        Some(addr.clone())
                    } else {
                        None
                    }
                }).collect::<Vec<_>>();
                for addr in timeout_clients {
                    if let Some(slot) = client_sockets.remove(&addr) {
                        log::info!("Remove timeout client socket {}", addr);
                        sessions.remove(&slot.session_id);
                        let hook_sender = hook_sender.clone();
                        async_std::task::spawn(async move {
                            hook_sender.hook_unregister(SipIncomingUnregisterRequest {
                                username: slot.username,
                                session_id: slot.session_id,
                                realm: slot.realm,
                            }).await.log_error("Should send hook_unregister");
                        });
                    }
                }
                continue;
            },
            e = sip_server.recv().fuse() => match e {
                Ok(event) => match event {
                    SipServerSocketMessage::RegisterValidate(group_id, digest, nonce, username, realm, hashed_password) => {
                        if let Some(slot) = client_sockets.get_mut(&group_id.addr()) {
                            log::info!("Register validate {} {} for cached user", username, hashed_password);
                            let hd2 = md5::compute(format!("REGISTER:{}", digest));
                            let hd2_str = format!("{:x}", hd2);
                            let response = md5::compute(format!("{}:{}:{}", slot.ha1_hash, nonce, hd2_str));
                            let response_str = format!("{:x}", response);
                            log::info!("Register local calculated md5 hash: {}:{}:{} => {} vs {}", slot.ha1_hash, nonce, hd2_str, response_str, hashed_password);
                            if hashed_password.eq(&response_str) {
                                slot.last_ts = timer.now_ms();
                                sip_server.accept_register(&group_id);
                            } else {
                                sip_server.reject_register(&group_id);
                            }
                        } else {
                            log::info!("Register validate {} {}", username, hashed_password);
                            let session_id = ctx.generate_conn_id();
                            let hook_sender = hook_sender.clone();
                            let internal_tx = internal_tx.clone();
                            async_std::task::spawn(async move {
                                log::info!("Register validate {} {} send hook_auth", username, hashed_password);
                                let res = hook_sender.hook_auth(SipIncomingAuthRequest {
                                    username: username.clone(),
                                    session_id: session_id.clone(),
                                    realm: realm.clone(),
                                }).await;

                                let (ha1_hash, accept) = match res {
                                    Ok(res) => match (res.success, res.ha1) {
                                        (true, Some(ha1)) => {
                                            let hd2 = md5::compute(format!("REGISTER:{}", digest));
                                            let hd2_str = format!("{:x}", hd2);
                                            let response = md5::compute(format!("{}:{}:{}", ha1, nonce, hd2_str));
                                            let response_str = format!("{:x}", response);
                                            log::info!("Register local calculated md5 hash: {}:{}:{} => {} vs {}", ha1, nonce, hd2_str, response_str, hashed_password);
                                            (Some(ha1), hashed_password.eq(&response_str))
                                        }
                                        _ => {
                                            log::info!("Register validate failed");
                                            (None, false)
                                        }
                                    },
                                    Err(e) => {
                                        log::error!("Error on hook register {:?}", e);
                                        (None, false)
                                    }
                                };

                                internal_tx.send(InternalCmd::RegisterResult(session_id.clone(), username.clone(), realm.clone(), ha1_hash, group_id, accept)).await.log_error("should send");
                                if accept {
                                    hook_sender.hook_register(SipIncomingRegisterRequest {
                                        username,
                                        session_id,
                                        realm,
                                    }).await.log_error("Should send register hook");
                                }
                            });
                        }
                        continue;
                    }
                    SipServerSocketMessage::InCall(socket, req) => {
                        run_incoming_call(sip_addr, hook_sender.clone(), socket, req, &ctx, timer.clone(), tx.clone());
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
                    InternalCmd::RegisterResult(session_id, username, realm, ha1_hash, group_id, result) => {
                        if result {
                            if let Some(slot) = client_sockets.get_mut(&group_id.addr()) {
                                slot.last_ts = timer.now_ms();
                            } else {
                                client_sockets.insert(group_id.addr(), ClientSockInfo {
                                    ha1_hash: ha1_hash.expect(""),
                                    username: username.clone(),
                                    realm: realm.clone(),
                                    session_id: session_id.clone(),
                                    last_ts: timer.now_ms(),
                                });
                                sessions.insert(session_id, ClientInfo {
                                    username,
                                    addr: group_id.addr(),
                                });
                            }
                            sip_server.accept_register(&group_id);
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
                RpcEvent::InviteOutgoingClient(req) => {
                    let dest_session_id = req.param().dest_session_id.clone();
                    if let Some(client_info) = sessions.get(&dest_session_id) {
                        let param = req.param().clone();
                        let room_id = param.room_id;
                        let from_number = param.from_number.clone();
                        let to_number = client_info.username.clone();
                        let to_addr = client_info.addr;

                        req.answer(run_outgoing_call(sip_addr, &mut sip_server, room_id, from_number, to_number, to_addr, &ctx, timer.clone(), tx.clone()));
                    } else {
                        req.answer(Err("NOT_FOUND"));
                    }
                }
                RpcEvent::InviteOutgoingServer(req) => {
                    let param = req.param().clone();
                    let room_id = param.room_id;
                    let from_number = param.from_number;
                    let to_number = param.to_number;
                    if let Ok(to_addr) = param.dest_addr.parse() {
                        req.answer(run_outgoing_call(sip_addr, &mut sip_server, room_id, from_number, to_number, to_addr, &ctx, timer.clone(), tx.clone()));
                    } else {
                        req.answer(Err("INVALID_DEST_ADDR"));
                    }
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
