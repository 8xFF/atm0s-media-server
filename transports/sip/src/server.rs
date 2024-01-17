use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    time::Duration,
};

use async_std::{net::UdpSocket, stream::Interval};
use futures::{select, FutureExt, StreamExt};
use media_utils::{SystemTimer, Timer};
use rsip::headers::CallId;

//TODO implement simple firewall here for blocking some ip addresses

use crate::{
    sip_request::SipRequest,
    sip_response::SipResponse,
    virtual_socket::{VirtualSocket, VirtualSocketContext, VirtualSocketPlane},
    GroupId, SipCore, SipMessage, SipServerEvent,
};

#[derive(Debug)]
pub enum SipServerSocketError {
    MessageParseError,
    NetworkError(std::io::Error),
}

pub enum SipServerSocketMessage {
    Continue,
    RegisterValidate(GroupId, String, String, String, String, String),
    InCall(VirtualSocket<GroupId, SipMessage>, SipRequest),
}

struct RemoteInfo {
    username: String,
    accepted: bool,
}

pub struct SipServerSocket {
    main_socket: UdpSocket,
    buf: [u8; 2048],
    sip_core: SipCore,
    timer: SystemTimer,
    interval: Interval,
    remote_users: HashMap<SocketAddr, RemoteInfo>,
    virtual_socket_plane: VirtualSocketPlane<GroupId, SipMessage>,
}

impl SipServerSocket {
    pub async fn new(bind_addr: SocketAddr) -> Self {
        log::info!("Listerning on port 5060 for UDP");
        Self {
            main_socket: UdpSocket::bind(bind_addr).await.expect("Should bind udp socket"),
            buf: [0u8; 2048],
            sip_core: SipCore::new(),
            timer: SystemTimer(),
            interval: async_std::stream::interval(Duration::from_millis(100)),
            remote_users: HashMap::new(),
            virtual_socket_plane: Default::default(),
        }
    }

    pub fn accept_register(&mut self, session: &GroupId) {
        log::info!("Accept register {:?}", session);
        if let Some(remote_info) = self.remote_users.get_mut(&session.addr()) {
            remote_info.accepted = true;
        }
        self.sip_core.reply_register_validate(session, true);
    }

    pub fn reject_register(&mut self, session: &GroupId) {
        log::info!("Reject register {:?}", session);
        self.remote_users.remove(&session.addr());
        self.sip_core.reply_register_validate(session, false);
    }

    pub fn create_call(&mut self, call_id: &CallId, dest: SocketAddr) -> VirtualSocket<GroupId, SipMessage> {
        let group_id = GroupId::from_raw(dest, &call_id);
        self.sip_core.open_out_call(&group_id);
        self.virtual_socket_plane.new_socket(group_id, VirtualSocketContext { remote_addr: dest, username: None })
    }

    pub async fn recv(&mut self) -> Result<SipServerSocketMessage, SipServerSocketError> {
        while let Some(output) = self.sip_core.pop_action() {
            match output {
                SipServerEvent::OnRegisterValidate(group, digest, nonce, username, realm, hashed_password) => {
                    log::info!("Register validate {} {}", username, hashed_password);
                    self.remote_users.insert(
                        group.addr(),
                        RemoteInfo {
                            username: username.clone(),
                            accepted: false,
                        },
                    );
                    return Ok(SipServerSocketMessage::RegisterValidate(group, digest, nonce, username, realm, hashed_password));
                }
                SipServerEvent::OnInCallStarted(group_id, req) => {
                    log::info!("InCall started {:?}", group_id);
                    let ctx = VirtualSocketContext {
                        remote_addr: group_id.addr(),
                        username: match self.remote_users.get(&group_id.addr()) {
                            Some(remote_info) => {
                                if remote_info.accepted {
                                    Some(remote_info.username.clone())
                                } else {
                                    None
                                }
                            }
                            None => None,
                        },
                    };
                    let socket = self.virtual_socket_plane.new_socket(group_id, ctx);
                    return Ok(SipServerSocketMessage::InCall(socket, req));
                }
                SipServerEvent::OnInCallRequest(group_id, req) => {
                    self.virtual_socket_plane
                        .forward(&group_id, SipMessage::Request(req))
                        .expect("Should forward to correct virtual socket");
                }
                SipServerEvent::OnOutCallRequest(group_id, req) => {
                    self.virtual_socket_plane
                        .forward(&group_id, SipMessage::Request(req))
                        .expect("Should forward to correct virtual socket");
                }
                SipServerEvent::OnOutCallResponse(group_id, res) => {
                    self.virtual_socket_plane
                        .forward(&group_id, SipMessage::Response(res))
                        .expect("Should forward to correct virtual socket");
                }
                SipServerEvent::SendRes(dest, res) => {
                    let buf = res.to_bytes();
                    log::info!("Send res to {} {}", dest, String::from_utf8_lossy(&buf));
                    if let Err(e) = self.main_socket.send_to(&buf, dest).await {
                        log::error!("Sending udp to {dest} error {:?}", e);
                    }
                }
                SipServerEvent::SendReq(dest, req) => {
                    let buf = req.to_bytes();
                    log::debug!("Send req to {} {}", dest, String::from_utf8_lossy(&buf));
                    if let Err(e) = self.main_socket.send_to(&buf, dest).await {
                        log::error!("Sending udp to {dest} error {:?}", e);
                    }
                }
                _ => {}
            }
        }

        let mut out_msgs = VecDeque::new();
        select! {
            _ = self.interval.next().fuse() => {
                self.sip_core.on_tick(self.timer.now_ms());
            },
            e = self.virtual_socket_plane.recv().fuse() => {
                match e {
                    Some((group_id, msg)) => {
                        match msg {
                            Some((dest, msg)) => {
                                let dest = dest.unwrap_or(group_id.addr());
                                log::info!("Group {:?} send to {} {}", group_id, dest, msg);
                                out_msgs.push_back((dest, msg));
                            },
                            None => {
                                log::info!("Group {:?} close socket", group_id);
                                self.virtual_socket_plane.close_socket(&group_id);
                                self.sip_core.close_in_call(&group_id);
                                self.sip_core.close_out_call(&group_id);
                            }
                        }
                    },
                    None => {}
                }
            },
            e = self.main_socket.recv_from(&mut self.buf).fuse() => {
                match e {
                    Ok((0..=4, addr)) => {
                        log::info!("Ping from {}", addr);
                    }
                    Ok((len, addr)) => {
                        log::info!("Recv from {}\n{}", addr, String::from_utf8(self.buf[..len].to_vec()).unwrap());
                        let req = match rsip::SipMessage::try_from(&self.buf[..len]) {
                            Ok(req) => req,
                            Err(e) => {
                                log::warn!("Can not parse request: {} {:?}", e, &self.buf[..len]);
                                return Err(SipServerSocketError::MessageParseError);
                            }
                        };

                        match req {
                            rsip::SipMessage::Request(req) => {
                                match SipRequest::from(req) {
                                    Ok(req) => {
                                        log::debug!("on req from {} {}", addr, req.method());
                                        if let Err(e) = self.sip_core.on_req(self.timer.now_ms(), addr, req) {
                                            log::error!("Process sip request error {:?}", e);
                                        }
                                    },
                                    Err(e) => {
                                        log::warn!("Can not parse request: {:?}", e);
                                        return Err(SipServerSocketError::MessageParseError);
                                    }
                                }
                            }
                            rsip::SipMessage::Response(res) => {
                                match SipResponse::from(res) {
                                    Ok(res) => {
                                        log::info!("on res from {} {}", addr, res.raw.status_code());
                                        if let Err(e) = self.sip_core.on_res(self.timer.now_ms(), addr, res) {
                                            log::error!("Process sip response error {:?}", e);
                                        }
                                    },
                                    Err(e) => {
                                        log::warn!("Can not parse response: {:?}", e);
                                        return Err(SipServerSocketError::MessageParseError);
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        log::warn!("Can not recv_from: {}", e);
                        return Err(SipServerSocketError::NetworkError(e));
                    }
                };
            }
        };

        while let Some((dest, msg)) = out_msgs.pop_front() {
            let buf = msg.to_bytes();
            log::info!("Send to {}\n{}", dest, String::from_utf8_lossy(&buf));
            if let Err(e) = self.main_socket.send_to(&buf, dest).await {
                log::error!("Sending udp to {dest} error {:?}", e);
            }
        }

        Ok(SipServerSocketMessage::Continue)
    }
}
