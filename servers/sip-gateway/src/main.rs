use std::{sync::Arc, time::Duration};

use async_std::net::UdpSocket;
use futures::{select, FutureExt, StreamExt};
use media_utils::{SystemTimer, Timer};
use rsip::SipMessage;
use transport_sip::{sip_request::SipRequest, sip_response::SipResponse, SipServer, SipServerEvent};

#[async_std::main]
async fn main() {
    env_logger::builder().format_timestamp_millis().init();
    log::info!("Listerning on port 5060 for UDP");
    let udp_listener = UdpSocket::bind("0.0.0.0:5060").await.expect("Should bind udp socket");
    let mut sip_server = SipServer::new();
    let timer = Arc::new(SystemTimer());
    let mut interval = async_std::stream::interval(Duration::from_millis(100));

    let mut buf = [0u8; 2048];
    loop {
        select! {
            _ = interval.next().fuse() => {
                sip_server.on_tick(timer.now_ms());
            },
            e = udp_listener.recv_from(&mut buf).fuse() => {
                let (len, addr) = match e {
                    Ok((len, addr)) => (len, addr),
                    Err(e) => {
                        log::warn!("Can not recv_from: {}", e);
                        continue;
                    }
                };

                // TODO limit source
                let req = match rsip::SipMessage::try_from(&buf[..len]) {
                    Ok(req) => req,
                    Err(e) => {
                        log::warn!("Can not parse request: {}", e);
                        continue;
                    }
                };

                match req {
                    SipMessage::Request(req) => {
                        match SipRequest::from(req) {
                            Ok(req) => {
                                log::info!("on req from {} {:?}", addr, req);
                                sip_server.on_req(timer.now_ms(), addr, req);
                            },
                            Err(e) => {
                                log::warn!("Can not parse request: {:?}", e);
                                continue;
                            }
                        }
                    }
                    SipMessage::Response(res) => {
                        match SipResponse::from(res) {
                            Ok(res) => {
                                log::info!("on res from {} {:?}", addr, res);
                                sip_server.on_res(timer.now_ms(), addr, res);
                            },
                            Err(e) => {
                                log::warn!("Can not parse response: {:?}", e);
                                continue;
                            }
                        }
                    }
                }

                while let Some(output) = sip_server.pop_action() {
                    match output {
                        SipServerEvent::OnRegisterValidate(group, username) => {
                            if username.starts_with("100") {
                                log::info!("Register validate from username {} => accept", username);
                                sip_server.reply_register_validate(group, true);
                            } else {
                                log::info!("Register validate from username {} => reject", username);
                                sip_server.reply_register_validate(group, false);
                            }
                        },
                        SipServerEvent::SendRes(dest, res) => {
                            log::info!("Send res to {} {:?}", dest, res);
                            udp_listener.send_to(&res.to_bytes(), dest).await;
                        },
                        SipServerEvent::SendReq(dest, req) => {
                            log::info!("Send req to {} {:?}", dest, req);
                            udp_listener.send_to(&req.to_bytes(), dest).await;
                        },
                        _ => {}
                    }
                }
            }
        }
    }
}
