use std::{collections::HashMap, sync::Arc};

use async_std::channel::{bounded, Sender};
use cluster::{Cluster, ClusterRoom};
use endpoint::MediaEndpointPreconditional;
use futures::{select, FutureExt};
use parking_lot::RwLock;
use utils::ServerError;

use crate::{
    rpc::{RpcEvent, RpcResponse, WhipConnectResponse},
    transport::life_cycle::whip::WhipTransportLifeCycle,
    transport::{WebrtcTransport, WebrtcTransportEvent},
};

enum InternalControl {
    RemoteIce(String, RpcResponse<()>),
}

pub struct WebrtcServer<C, CR> {
    _tmp_cr: std::marker::PhantomData<CR>,
    cluster: C,
    conns: Arc<RwLock<HashMap<u64, Sender<InternalControl>>>>,
}

impl<C, CR: 'static> WebrtcServer<C, CR>
where
    C: Cluster<CR>,
    CR: ClusterRoom,
{
    pub fn new(cluster: C) -> Self {
        Self {
            _tmp_cr: std::marker::PhantomData,
            cluster,
            conns: Arc::new(RwLock::new(HashMap::<u64, Sender<InternalControl>>::new())),
        }
    }

    pub fn on_incomming(&mut self, event: RpcEvent) {
        match event {
            RpcEvent::WhipConnect(_token, sdp, mut res) => {
                let room = self.cluster.build();
                let conns_c = self.conns.clone();
                async_std::task::spawn(async move {
                    let mut endpoint_pre = MediaEndpointPreconditional::new();
                    if let Err(err) = endpoint_pre.check() {
                        res.answer(200, Err(err));
                        return;
                    }

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
                    let conn_id = 0; //TODO generate this
                    res.answer(
                        200,
                        Ok(WhipConnectResponse {
                            location: "/location_here".to_string(),
                            sdp: answer_sdp,
                        }),
                    );
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
                                        res.answer(200, Err(ServerError::build("REMOTE_ICE_ERROR", err)));
                                        return;
                                    }
                                    res.answer(200, Ok(()));
                                }
                                _ => {}
                            }
                        }
                    }
                });
            }
            RpcEvent::Connect(sdp, mut res) => {
                res.answer(404, Err(ServerError::build("NOT_IMPLEMENTED", "")));
            }
            RpcEvent::RemoteIce(conn_id, ice, res) => {
                if let Some(tx) = self.conns.read().get(&conn_id) {
                    if let Err(_e) = tx.send_blocking(InternalControl::RemoteIce(ice, res)) {
                        //TODO handle this
                    };
                }
            }
        }
    }
}
