use std::sync::Arc;

use cluster::{
    implement::NodeId,
    rpc::{
        connector::{MediaEndpointEvent, MediaEndpointLogRequest, MediaEndpointLogResponse},
        gateway::{NodeHealthcheckRequest, NodeHealthcheckResponse},
        RpcEmitter, RpcReqRes, RPC_MEDIA_ENDPOINT_LOG, RPC_NODE_HEALTHCHECK,
    },
    CONNECTOR_SERVICE, MEDIA_SERVER_SERVICE,
};
use futures::FutureExt as _;
use media_utils::{ErrorDebugger, Timer};
use metrics::increment_counter;

use crate::server::gateway::{GATEWAY_SESSIONS_CONNECT_COUNT, GATEWAY_SESSIONS_CONNECT_ERROR};

use super::logic::{GatewayLogic, ServiceType};

async fn select_node<EMITTER: RpcEmitter + Send + 'static>(emitter: &EMITTER, node_ids: &[u32]) -> Option<u32> {
    let mut futures = Vec::new();

    for node_id in node_ids {
        let future = emitter
            .request::<_, NodeHealthcheckResponse>(
                MEDIA_SERVER_SERVICE,
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
fn emit_endpoint_event<EMITTER: RpcEmitter + Send + 'static>(emitter: &EMITTER, timer: &Arc<dyn Timer>, session_uuid: u64, event: MediaEndpointEvent) {
    let emitter = emitter.clone();
    let ts = timer.now_ms();
    async_std::task::spawn_local(async move {
        emitter
            .request::<_, MediaEndpointLogResponse>(
                CONNECTOR_SERVICE,
                None,
                RPC_MEDIA_ENDPOINT_LOG,
                MediaEndpointLogRequest::SessionEvent {
                    ip: "127.0.0.1".to_string(), //TODO get real ip
                    location: None,
                    version: None,
                    token: vec![],
                    ts,
                    session_uuid,
                    event,
                },
                1000,
            )
            .await
            .log_error("Should ok");
    });
}

pub fn route_to_node<EMITTER, Req, Res>(
    emitter: EMITTER,
    timer: Arc<dyn Timer>,
    gateway_logic: &mut GatewayLogic,
    gateway_node_id: NodeId,
    service: ServiceType,
    cmd: &'static str,
    req: Box<dyn RpcReqRes<Req, Res> + Sync>,
) where
    EMITTER: RpcEmitter + Send + Sync + 'static,
    Req: Into<Vec<u8>> + Send + Sync + Clone + 'static,
    Res: for<'a> TryFrom<&'a [u8]> + Send + 'static,
{
    increment_counter!(GATEWAY_SESSIONS_CONNECT_COUNT);
    let started_ms = timer.now_ms();
    let session_uuid = 0; //TODO
    let event = MediaEndpointEvent::Routing {
        user_agent: "TODO".to_string(),
        gateway_node_id,
    };
    emit_endpoint_event(&emitter, &timer, session_uuid, event);

    let nodes = gateway_logic.best_nodes(service, 60, 80, 3);
    if !nodes.is_empty() {
        let rpc_emitter = emitter.clone();
        async_std::task::spawn(async move {
            log::info!("[Gateway] connect => ping nodes {:?}", nodes);
            let node_id = select_node(&rpc_emitter, &nodes).await;
            if let Some(node_id) = node_id {
                log::info!("[Gateway] connect with selected node {:?}", node_id);
                let res = rpc_emitter.request::<Req, Res>(MEDIA_SERVER_SERVICE, Some(node_id), cmd, req.param().clone(), 5000).await;
                log::info!("[Gateway] webrtc connect res from media-server {:?}", res.as_ref().map(|_| ()));
                let event = if res.is_err() {
                    increment_counter!(GATEWAY_SESSIONS_CONNECT_ERROR);
                    MediaEndpointEvent::RoutingError {
                        reason: "NODE_ANSWER_ERROR".to_string(),
                        gateway_node_id,
                        media_node_ids: vec![node_id],
                    }
                } else {
                    MediaEndpointEvent::Routed {
                        media_node_id: node_id,
                        after_ms: (timer.now_ms() - started_ms) as u32,
                    }
                };

                emit_endpoint_event(&emitter, &timer, session_uuid, event);
                req.answer(res.map_err(|_e| "NODE_ANSWER_ERROR"));
            } else {
                log::warn!("[Gateway] webrtc connect but ping nodes {:?} timeout", nodes);
                increment_counter!(GATEWAY_SESSIONS_CONNECT_ERROR);
                let event = MediaEndpointEvent::RoutingError {
                    reason: "NODE_PING_TIMEOUT".to_string(),
                    gateway_node_id,
                    media_node_ids: nodes,
                };
                emit_endpoint_event(&emitter, &timer, session_uuid, event);
                req.answer(Err("NODE_PING_TIMEOUT"));
            }
        });
    } else {
        increment_counter!(GATEWAY_SESSIONS_CONNECT_ERROR);
        let event = MediaEndpointEvent::RoutingError {
            reason: "NODE_POOL_EMPTY".to_string(),
            gateway_node_id,
            media_node_ids: vec![],
        };
        emit_endpoint_event(&emitter, &timer, session_uuid, event);

        log::warn!("[Gateway] webrtc connect but media-server pool empty");
        req.answer(Err("NODE_POOL_EMPTY"));
    }
}
