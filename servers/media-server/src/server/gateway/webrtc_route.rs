use std::{net::IpAddr, sync::Arc};

use cluster::{
    implement::NodeId,
    rpc::{
        connector::MediaEndpointLogResponse,
        gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
        RpcEmitter, RpcReqRes, RPC_MEDIA_ENDPOINT_LOG, RPC_NODE_HEALTHCHECK,
    },
    CONNECTOR_SERVICE,
};
use futures::FutureExt as _;
use media_utils::{ErrorDebugger, Timer, F32};
use metrics::counter;
use protocol::media_event_logs::{
    session_event::{SessionRouted, SessionRouting, SessionRoutingError},
    MediaEndpointLogEvent, MediaSessionEvent, SessionEvent,
};

use crate::server::gateway::{GATEWAY_SESSIONS_CONNECT_COUNT, GATEWAY_SESSIONS_CONNECT_ERROR};

use super::logic::{GatewayLogic, RouteResult, ServiceType};

async fn select_node<EMITTER: RpcEmitter + Send + 'static>(emitter: &EMITTER, node_ids: &[u32], service_id: u8) -> Option<u32> {
    let mut futures = Vec::new();

    for node_id in node_ids {
        let future = emitter
            .request::<_, NodeHealthcheckResponse>(
                service_id,
                Some(*node_id),
                RPC_NODE_HEALTHCHECK,
                NodeHealthcheckRequest::Webrtc {
                    max_send_bitrate: 2_000_000,
                    max_recv_bitrate: 2_000_000,
                },
                1000,
            )
            .map(move |res| match res {
                Ok(res) => {
                    log::info!("on res {:?}", res);
                    if res.success {
                        Ok(*node_id)
                    } else {
                        Err(())
                    }
                }
                Err(_) => Err(()),
            });
        futures.push(future);
    }

    let first_completed = futures::future::select_ok(futures).await;
    first_completed.ok().map(|(node_id, _)| node_id)
}

// TODO running in queue and retry if failed. It should retry when connector service not accept
fn emit_endpoint_event<EMITTER: RpcEmitter + Send + 'static>(emitter: &EMITTER, timer: &Arc<dyn Timer>, session_uuid: u64, ip: &str, version: &Option<String>, event: MediaSessionEvent) {
    let emitter = emitter.clone();
    let ts = timer.now_ms();
    let ip = ip.to_string();
    let version = version.clone();
    async_std::task::spawn_local(async move {
        emitter
            .request::<_, MediaEndpointLogResponse>(
                CONNECTOR_SERVICE,
                None,
                RPC_MEDIA_ENDPOINT_LOG,
                MediaEndpointLogEvent::SessionEvent(SessionEvent {
                    ip,
                    location: None,
                    version,
                    token: vec![],
                    ts,
                    session_uuid,
                    event: Some(event),
                }),
                1000,
            )
            .await
            .log_error("Should send media-log-event to connector");
    });
}

pub fn route_to_node<EMITTER, Req, Res>(
    emitter: EMITTER,
    timer: Arc<dyn Timer>,
    gateway_logic: &mut GatewayLogic,
    gateway_node_id: NodeId,
    service: ServiceType,
    cmd: &'static str,
    ip: IpAddr,
    location: Option<(F32<2>, F32<2>)>,
    version: &Option<String>,
    user_agent: &str,
    session_uuid: u64,
    req: Box<dyn RpcReqRes<Req, Res>>,
) where
    EMITTER: RpcEmitter + Send + Sync + 'static,
    Req: Into<Vec<u8>> + Send + Clone + 'static,
    Res: for<'a> TryFrom<&'a [u8]> + Send + 'static,
{
    counter!(GATEWAY_SESSIONS_CONNECT_COUNT).increment(1);
    let started_ms = timer.now_ms();
    let event = MediaSessionEvent::Routing(SessionRouting {
        user_agent: user_agent.to_string(),
        gateway_node_id,
    });
    emit_endpoint_event(&emitter, &timer, session_uuid, &ip.to_string(), version, event);

    let route_res = gateway_logic.best_nodes(location, service, 60, 80, 3);
    if let RouteResult::OtherNode { nodes, service_id } = route_res {
        let rpc_emitter = emitter.clone();
        let ip: String = ip.to_string();
        let version = version.clone();
        let param = req.param().clone();
        async_std::task::spawn(async move {
            log::info!("[Gateway] connect => ping nodes {:?}", nodes);
            let node_id = select_node(&rpc_emitter, &nodes, service_id).await;
            if let Some(node_id) = node_id {
                log::info!("[Gateway] connect with selected node {:?}", node_id);
                let res = rpc_emitter.request::<Req, Res>(service_id, Some(node_id), cmd, param, 5000).await;
                log::info!("[Gateway] webrtc connect res from media-server {:?}", res.as_ref().map(|_| ()));
                let event = if res.is_err() {
                    counter!(GATEWAY_SESSIONS_CONNECT_ERROR).increment(1);
                    MediaSessionEvent::RoutingError(SessionRoutingError {
                        reason: "NODE_ANSWER_ERROR".to_string(),
                        gateway_node_id,
                        media_node_ids: vec![node_id],
                    })
                } else {
                    MediaSessionEvent::Routed(SessionRouted {
                        media_node_id: node_id,
                        after_ms: (timer.now_ms() - started_ms) as u32,
                    })
                };

                emit_endpoint_event(&emitter, &timer, session_uuid, &ip, &version, event);
                req.answer(res.map_err(|_e| "NODE_ANSWER_ERROR"));
            } else {
                log::warn!("[Gateway] webrtc connect but ping nodes {:?} timeout", nodes);
                counter!(GATEWAY_SESSIONS_CONNECT_ERROR).increment(1);
                let event = MediaSessionEvent::RoutingError(SessionRoutingError {
                    reason: "NODE_PING_TIMEOUT".to_string(),
                    gateway_node_id,
                    media_node_ids: nodes,
                });
                emit_endpoint_event(&emitter, &timer, session_uuid, &ip, &version, event);
                req.answer(Err("NODE_PING_TIMEOUT"));
            }
        });
    } else {
        counter!(GATEWAY_SESSIONS_CONNECT_ERROR).increment(1);
        let event = MediaSessionEvent::RoutingError(SessionRoutingError {
            reason: "NODE_POOL_EMPTY".to_string(),
            gateway_node_id,
            media_node_ids: vec![],
        });

        emit_endpoint_event(&emitter, &timer, session_uuid, &ip.to_string(), version, event);

        log::warn!("[Gateway] webrtc connect but media-server pool empty");
        req.answer(Err("NODE_POOL_EMPTY"));
    }
}
