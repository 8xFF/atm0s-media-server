use std::{collections::HashMap, sync::Arc};

use async_std::channel::{bounded, Sender};
use cluster_local::ServerLocal;
use endpoint::MediaEndpointPreconditional;
use futures::{select, FutureExt};
use parking_lot::RwLock;
use utils::ServerError;

mod transport;

use crate::transport::{WebrtcTransport, WebrtcTransportEvent};

struct HttpResponse {}

impl HttpResponse {
    pub fn answer(&mut self, code: u16, body: Result<String, ServerError>) {}
}

enum HttpEvent {
    Connect(String, HttpResponse),
    RemoteIce(u64, String, HttpResponse),
}

enum InternalControl {
    RemoteIce(String, HttpResponse),
}

#[async_std::main]
async fn main() {
    let (tx, rx) = bounded(1);
    let mut cluster = ServerLocal::new();
    let conns = Arc::new(RwLock::new(HashMap::<u64, Sender<InternalControl>>::new()));
    while let Ok(event) = rx.recv().await {
        match event {
            HttpEvent::Connect(sdp, mut res) => {
                let room = cluster.build();
                let conns_c = conns.clone();
                async_std::task::spawn(async move {
                    let mut endpoint_pre = MediaEndpointPreconditional::new();
                    if let Err(err) = endpoint_pre.check() {
                        res.answer(200, Err(err));
                        return;
                    }

                    let mut transport = match WebrtcTransport::new().await {
                        Ok(transport) => transport,
                        Err(err) => {
                            res.answer(
                                200,
                                Err(ServerError {
                                    code: "500".to_string(),
                                    message: err.to_string(),
                                }),
                            );
                            return;
                        }
                    };
                    let answer_sdp = match transport.on_remote_sdp(&sdp) {
                        Ok(sdp) => sdp,
                        Err(err) => {
                            res.answer(
                                200,
                                Err(ServerError {
                                    code: "ANSWER_SDP_ERROR".to_string(),
                                    message: err.to_string(),
                                }),
                            );
                            return;
                        }
                    };
                    let conn_id = 0; //TODO generate this
                    res.answer(200, Ok(answer_sdp));
                    let (tx, rx) = bounded(1);
                    conns_c.write().insert(conn_id, tx);

                    let mut endpoint = endpoint_pre.build(transport, room);

                    loop {
                        select! {
                            e = endpoint.recv().fuse() => match e {
                                Ok(_) => {},
                                Err(e) => {
                                    break;
                                }
                            },
                            e = rx.recv().fuse() => match e {
                                Ok(InternalControl::RemoteIce(ice, mut res)) => {
                                    if let Err(err) = endpoint.on_custom_event(WebrtcTransportEvent::RemoteIce(ice)) {
                                        res.answer(200, Err(ServerError { code: "REMOTE_ICE_ERROR".to_string(), message: err.to_string() }));
                                        return;
                                    }
                                    res.answer(200, Ok("".to_string()));
                                }
                                _ => {}
                            }
                        }
                    }
                });
            }
            HttpEvent::RemoteIce(conn_id, ice, res) => {
                if let Some(tx) = conns.read().get(&conn_id) {
                    if let Err(_e) = tx.send(InternalControl::RemoteIce(ice, res)).await {
                        //TODO handle this
                    };
                }
            }
        }
    }
}
