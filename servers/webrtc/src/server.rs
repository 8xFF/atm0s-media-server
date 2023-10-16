use std::{collections::HashMap, sync::Arc};

use async_std::channel::{bounded, Sender};
use cluster::{Cluster, ClusterEndpoint};
use endpoint::MediaEndpointPreconditional;
use futures::{select, FutureExt};
use parking_lot::RwLock;
use utils::{ServerError, Timer};

use crate::{
    rpc::{RpcEvent, RpcResponse, WebrtcConnectResponse, WhipConnectResponse},
    transport::internal::life_cycle::{sdk::SdkTransportLifeCycle, whip::WhipTransportLifeCycle},
    transport::{WebrtcTransport, WebrtcTransportEvent},
};

enum InternalControl {
    RemoteIce(String, RpcResponse<()>),
}

pub struct WebrtcServer<C, CR> {
    _tmp_cr: std::marker::PhantomData<CR>,
    cluster: C,
    counter: u64,
    conns: Arc<RwLock<HashMap<String, Sender<InternalControl>>>>,
    timer: Arc<dyn Timer>,
}

impl<C, CR: 'static> WebrtcServer<C, CR>
where
    C: Cluster<CR>,
    CR: ClusterEndpoint,
{
    pub fn new(cluster: C) -> Self {
        Self {
            _tmp_cr: std::marker::PhantomData,
            cluster,
            counter: 0,
            conns: Arc::new(RwLock::new(HashMap::new())),
            timer: Arc::new(utils::SystemTimer()),
        }
    }

    pub fn on_incomming(&mut self, event: RpcEvent) {
        match event {
            RpcEvent::WhipConnect(_token, sdp, mut res) => {
                let mut endpoint_pre = MediaEndpointPreconditional::new("room1", "peer1"); //TODO fill correct room and peer
                if let Err(err) = endpoint_pre.check() {
                    res.answer(200, Err(err));
                    return;
                }

                let connect_count = self.counter;
                self.counter += 1;
                let room = self.cluster.build("room1", "peer1"); //TODO fill correct room and peer
                let conns_c = self.conns.clone();
                async_std::task::spawn(async move {
                    let mut transport = match WebrtcTransport::new(WhipTransportLifeCycle::new()).await {
                        Ok(transport) => transport,
                        Err(err) => {
                            res.answer(200, Err(ServerError::build("500", err)));
                            return;
                        }
                    };
                    let answer_sdp = match transport.on_remote_sdp(&sdp) {
                        Ok(sdp) => sdp,
                        Err(err) => {
                            res.answer(200, Err(ServerError::build("ANSWER_SDP_ERROR", err)));
                            return;
                        }
                    };
                    let conn_id = format!("conn-{}", connect_count);
                    res.answer(
                        200,
                        Ok(WhipConnectResponse {
                            location: "/location_here".to_string(),
                            sdp: answer_sdp,
                        }),
                    );
                    let (tx, rx) = bounded(1);
                    conns_c.write().insert(conn_id.clone(), tx);

                    let mut endpoint = endpoint_pre.build(transport, room);

                    loop {
                        select! {
                            e = endpoint.recv().fuse() => match e {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error on endpoint recv: {:?}", e);
                                    break;
                                }
                            },
                            e = rx.recv().fuse() => match e {
                                Ok(InternalControl::RemoteIce(ice, mut res)) => {
                                    if let Err(err) = endpoint.on_custom_event(WebrtcTransportEvent::RemoteIce(ice)) {
                                        res.answer(200, Err(ServerError::build("REMOTE_ICE_ERROR", err)));
                                        return;
                                    }
                                    res.answer(200, Ok(()));
                                }
                                Err(e) => {
                                    log::error!("Error on endpoint custom recv: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }

                    conns_c.write().remove(&conn_id);
                });
            }
            RpcEvent::WebrtcConnect(req, mut res) => {
                let mut endpoint_pre = MediaEndpointPreconditional::new(&req.room, &req.peer);
                if let Err(err) = endpoint_pre.check() {
                    res.answer(200, Err(err));
                    return;
                }

                let now_ms = self.timer.now_ms();
                let room = self.cluster.build(&req.room, &req.peer);
                let connect_count = self.counter;
                self.counter += 1;
                let conns_c = self.conns.clone();
                async_std::task::spawn(async move {
                    let mut transport = match WebrtcTransport::new(SdkTransportLifeCycle::new(now_ms)).await {
                        Ok(transport) => transport,
                        Err(err) => {
                            log::error!("Transport error {:?}", err);
                            res.answer(200, Err(ServerError::build("500", err)));
                            return;
                        }
                    };

                    for sender in req.senders {
                        transport.map_remote_stream(sender);
                    }
                    let answer_sdp = match transport.on_remote_sdp(&req.sdp) {
                        Ok(sdp) => sdp,
                        Err(err) => {
                            log::error!("Transport error {:?}", err);
                            res.answer(200, Err(ServerError::build("ANSWER_SDP_ERROR", err)));
                            return;
                        }
                    };
                    let conn_id = format!("conn-{}", connect_count);
                    res.answer(
                        200,
                        Ok(WebrtcConnectResponse {
                            sdp: answer_sdp,
                            conn_id: conn_id.clone(),
                        }),
                    );
                    let (tx, rx) = bounded(1);
                    conns_c.write().insert(conn_id.clone(), tx);

                    let mut endpoint = endpoint_pre.build(transport, room);

                    log::info!("[WebrtcServer] start loop for endpoint");
                    loop {
                        select! {
                            e = endpoint.recv().fuse() => match e {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("Error on endpoint recv: {:?}", e);
                                    break;
                                }
                            },
                            e = rx.recv().fuse() => match e {
                                Ok(InternalControl::RemoteIce(ice, mut res)) => {
                                    if let Err(err) = endpoint.on_custom_event(WebrtcTransportEvent::RemoteIce(ice)) {
                                        res.answer(200, Err(ServerError::build("REMOTE_ICE_ERROR", err)));
                                        return;
                                    }
                                    res.answer(200, Ok(()));
                                }
                                Err(e) => {
                                    log::error!("Error on endpoint custom recv: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                    log::info!("[WebrtcServer] stop loop for endpoint");
                    conns_c.write().remove(&conn_id);
                });
            }
            RpcEvent::WebrtcRemoteIce(conn_id, ice, res) => {
                if let Some(tx) = self.conns.read().get(&conn_id) {
                    if let Err(_e) = tx.send_blocking(InternalControl::RemoteIce(ice, res)) {
                        //TODO handle this
                    };
                }
            }
        }
    }
}
