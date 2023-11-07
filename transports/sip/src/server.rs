use std::{collections::VecDeque, time::Duration};

use async_std::{net::UdpSocket, stream::Interval};
use futures::{select, FutureExt, StreamExt};
use media_utils::{SystemTimer, Timer};

use crate::{
    sip_request::SipRequest,
    sip_response::SipResponse,
    virtual_socket::{VirtualSocket, VirtualSocketPlane},
    GroupId, SipMessage, SipServer, SipServerEvent,
};

#[derive(Debug)]
pub enum SipServerSocketError {
    MessageParseError,
    NetworkError(std::io::Error),
}

pub enum SipServerSocketMessage {
    Continue,
    InCall(VirtualSocket<GroupId, SipMessage>, SipRequest),
}

pub struct SipServerSocket {
    main_socket: UdpSocket,
    buf: [u8; 2048],
    sip_server: SipServer,
    timer: SystemTimer,
    interval: Interval,
    virtual_socket_plane: VirtualSocketPlane<GroupId, SipMessage>,
}

impl SipServerSocket {
    pub async fn new() -> Self {
        log::info!("Listerning on port 5060 for UDP");
        Self {
            main_socket: UdpSocket::bind("0.0.0.0:5060").await.expect("Should bind udp socket"),
            buf: [0u8; 2048],
            sip_server: SipServer::new(),
            timer: SystemTimer(),
            interval: async_std::stream::interval(Duration::from_millis(100)),
            virtual_socket_plane: Default::default(),
        }
    }

    pub async fn recv(&mut self) -> Result<SipServerSocketMessage, SipServerSocketError> {
        let mut out_msgs = VecDeque::new();
        select! {
            _ = self.interval.next().fuse() => {
                self.sip_server.on_tick(self.timer.now_ms());
            },
            e = self.virtual_socket_plane.recv().fuse() => {
                match e {
                    Some((group_id, msg)) => {
                        match msg {
                            Some((dest, msg)) => {
                                let dest = dest.unwrap_or(group_id.0);
                                log::info!("Send to {} {}", dest, msg);
                                out_msgs.push_back((dest, msg));
                            },
                            None => {
                                self.virtual_socket_plane.close_socket(&group_id);
                                self.sip_server.close_in_call(&group_id);
                            }
                        }
                    },
                    None => {}
                }
            }
            e = self.main_socket.recv_from(&mut self.buf).fuse() => {
                match e {
                    Ok((0..=4, addr)) => {
                        log::info!("Ping from {}", addr);
                    }
                    Ok((len, addr)) => {
                        log::debug!("Recv from {}\n{}", addr, String::from_utf8(self.buf[..len].to_vec()).unwrap());
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
                                        log::info!("on req from {} {}", addr, req.method());
                                        self.sip_server.on_req(self.timer.now_ms(), addr, req);
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
                                        self.sip_server.on_res(self.timer.now_ms(), addr, res);
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
            log::debug!("Send to {}\n{}", dest, String::from_utf8(buf.to_vec()).unwrap());
            self.main_socket.send_to(&buf, dest).await;
        }

        while let Some(output) = self.sip_server.pop_action() {
            match output {
                SipServerEvent::OnRegisterValidate(group, username) => {
                    //TODO implement real logic here
                    if username.starts_with("100") {
                        log::info!("Register validate from username {} => accept", username);
                        self.sip_server.reply_register_validate(group, true);
                    } else {
                        log::info!("Register validate from username {} => reject", username);
                        self.sip_server.reply_register_validate(group, false);
                    }
                }
                SipServerEvent::OnInCallStarted(group_id, req) => {
                    log::info!("InCall started {:?}", group_id);
                    let socket = self.virtual_socket_plane.new_socket(group_id);
                    return Ok(SipServerSocketMessage::InCall(socket, req));
                }
                SipServerEvent::OnInCallRequest(group_id, req) => {
                    self.virtual_socket_plane.forward(&group_id, SipMessage::Request(req));
                }
                SipServerEvent::SendRes(dest, res) => {
                    log::info!("Send res to {} {}", dest, res.raw.status_code());
                    self.main_socket.send_to(&res.to_bytes(), dest).await;
                }
                SipServerEvent::SendReq(dest, req) => {
                    log::info!("Send req to {} {}", dest, req.method());
                    self.main_socket.send_to(&req.to_bytes(), dest).await;
                }
                _ => {}
            }
        }

        Ok(SipServerSocketMessage::Continue)
    }
}
