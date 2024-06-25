use std::{net::SocketAddr, sync::Arc};

use atm0s_sdn::NodeId;
use media_server_connector::agent_service::Control as ConnectorControl;
use media_server_gateway::ServiceKind;
use media_server_protocol::{
    endpoint::ClusterConnId,
    gateway::GATEWAY_RPC_PORT,
    protobuf::{
        cluster_connector::{
            connector_request::Event as ConnectorEvent,
            peer_event::{route_error::ErrorType, Event as PeerEvent2, RouteBegin, RouteError, RouteSuccess},
            PeerEvent,
        },
        cluster_gateway::{
            MediaEdgeServiceClient, MediaEdgeServiceHandler, WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse, WebrtcRestartIceRequest,
            WebrtcRestartIceResponse, WhepCloseRequest, WhepCloseResponse, WhepConnectRequest, WhepConnectResponse, WhepRemoteIceRequest, WhepRemoteIceResponse, WhipCloseRequest, WhipCloseResponse,
            WhipConnectRequest, WhipConnectResponse, WhipRemoteIceRequest, WhipRemoteIceResponse,
        },
    },
    rpc::{
        node_vnet_addr,
        quinn::{QuinnClient, QuinnStream},
    },
    transport::ConnLayer,
};
use media_server_utils::now_ms;
use tokio::sync::mpsc::Sender;

use super::{dest_selector::GatewayDestSelector, ip_location::Ip2Location};

#[derive(Clone)]
pub struct Ctx {
    pub(crate) connector_agent_tx: Sender<media_server_connector::agent_service::Control>,
    pub(crate) selector: GatewayDestSelector,
    pub(crate) client: MediaEdgeServiceClient<SocketAddr, QuinnClient, QuinnStream>,
    pub(crate) ip2location: Arc<Ip2Location>,
}

#[derive(Default)]
pub struct MediaRemoteRpcHandlerImpl {}

impl MediaRemoteRpcHandlerImpl {
    async fn feedback_route_begin(ctx: &Ctx, session_id: u64, remote_ip: String) {
        ctx.connector_agent_tx
            .send(ConnectorControl::Fire(
                now_ms(),
                ConnectorEvent::Peer(PeerEvent {
                    session_id,
                    event: Some(PeerEvent2::RouteBegin(RouteBegin { remote_ip })),
                }),
            ))
            .await
            .expect("Should send");
    }

    async fn feedback_route_success(ctx: &Ctx, session_id: u64, after_ms: u64, node: NodeId) {
        ctx.connector_agent_tx
            .send(ConnectorControl::Fire(
                now_ms(),
                ConnectorEvent::Peer(PeerEvent {
                    session_id,
                    event: Some(PeerEvent2::RouteSuccess(RouteSuccess {
                        after_ms: after_ms as u32,
                        dest_node: node,
                    })),
                }),
            ))
            .await
            .expect("Should send");
    }

    async fn feedback_route_error(ctx: &Ctx, session_id: u64, after_ms: u64, node: Option<NodeId>, error: ErrorType) {
        ctx.connector_agent_tx
            .send(ConnectorControl::Fire(
                now_ms(),
                ConnectorEvent::Peer(PeerEvent {
                    session_id,
                    event: Some(PeerEvent2::RouteError(RouteError {
                        after_ms: after_ms as u32,
                        dest_node: node,
                        error: error as i32,
                    })),
                }),
            ))
            .await
            .expect("Should send");
    }
}

impl MediaEdgeServiceHandler<Ctx> for MediaRemoteRpcHandlerImpl {
    async fn whip_connect(&self, ctx: &Ctx, req: WhipConnectRequest) -> Option<WhipConnectResponse> {
        let started_at = now_ms();
        let session_id = req.session_id;
        log::info!("On whip_connect from other gateway");
        Self::feedback_route_begin(ctx, session_id, req.ip.clone()).await;
        let location = req.ip.parse().ok().and_then(|ip| ctx.ip2location.get_location(&ip));
        if let Some(node_id) = ctx.selector.select(ServiceKind::Webrtc, location).await {
            let node_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            if let Some(res) = ctx.client.whip_connect(node_addr, req).await {
                Self::feedback_route_success(ctx, session_id, now_ms() - started_at, node_id).await;
                Some(res)
            } else {
                Self::feedback_route_error(ctx, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                None
            }
        } else {
            Self::feedback_route_error(ctx, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            None
        }
    }

    async fn whip_remote_ice(&self, ctx: &Ctx, req: WhipRemoteIceRequest) -> Option<WhipRemoteIceResponse> {
        log::info!("On whip_remote_ice from other gateway");
        let conn: ClusterConnId = req.conn.parse().ok()?;
        let (dest, _session) = conn.get_down_part();
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.whip_remote_ice(dest_addr, req).await
    }

    async fn whip_close(&self, ctx: &Ctx, req: WhipCloseRequest) -> Option<WhipCloseResponse> {
        log::info!("On whip_close from other gateway");
        let conn: ClusterConnId = req.conn.parse().ok()?;
        let (dest, _session) = conn.get_down_part();
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.whip_close(dest_addr, req).await
    }

    async fn whep_connect(&self, ctx: &Ctx, req: WhepConnectRequest) -> Option<WhepConnectResponse> {
        let started_at = now_ms();
        let session_id = req.session_id;
        log::info!("On whep_connect from other gateway");
        Self::feedback_route_begin(ctx, session_id, req.ip.clone()).await;
        let location = req.ip.parse().ok().and_then(|ip| ctx.ip2location.get_location(&ip));
        if let Some(node_id) = ctx.selector.select(ServiceKind::Webrtc, location).await {
            let dest_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            if let Some(res) = ctx.client.whep_connect(dest_addr, req).await {
                Self::feedback_route_success(ctx, session_id, now_ms() - started_at, node_id).await;
                Some(res)
            } else {
                Self::feedback_route_error(ctx, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                None
            }
        } else {
            Self::feedback_route_error(ctx, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            None
        }
    }

    async fn whep_remote_ice(&self, ctx: &Ctx, req: WhepRemoteIceRequest) -> Option<WhepRemoteIceResponse> {
        log::info!("On whep_remote_ice from other gateway");
        let conn: ClusterConnId = req.conn.parse().ok()?;
        let (dest, _session) = conn.get_down_part();
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.whep_remote_ice(dest_addr, req).await
    }

    async fn whep_close(&self, ctx: &Ctx, req: WhepCloseRequest) -> Option<WhepCloseResponse> {
        log::info!("On whep_close from other gateway");
        let conn: ClusterConnId = req.conn.parse().ok()?;
        let (dest, _session) = conn.get_down_part();
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.whep_close(dest_addr, req).await
    }

    async fn webrtc_connect(&self, ctx: &Ctx, req: WebrtcConnectRequest) -> Option<WebrtcConnectResponse> {
        let started_at = now_ms();
        let session_id = req.session_id;
        log::info!("On webrtc_connect from other gateway");
        Self::feedback_route_begin(ctx, session_id, req.ip.clone()).await;
        let location = req.ip.parse().ok().and_then(|ip| ctx.ip2location.get_location(&ip));
        if let Some(node_id) = ctx.selector.select(ServiceKind::Webrtc, location).await {
            let dest_addr = node_vnet_addr(node_id, GATEWAY_RPC_PORT);
            if let Some(res) = ctx.client.webrtc_connect(dest_addr, req).await {
                Self::feedback_route_success(ctx, session_id, now_ms() - started_at, node_id).await;
                Some(res)
            } else {
                Self::feedback_route_error(ctx, session_id, now_ms() - started_at, Some(node_id), ErrorType::Timeout).await;
                None
            }
        } else {
            Self::feedback_route_error(ctx, session_id, now_ms() - started_at, None, ErrorType::PoolEmpty).await;
            None
        }
    }

    async fn webrtc_remote_ice(&self, ctx: &Ctx, req: WebrtcRemoteIceRequest) -> Option<WebrtcRemoteIceResponse> {
        log::info!("On webrtc_remote_ice from other gateway");
        let conn: ClusterConnId = req.conn.parse().ok()?;
        let (dest, _session) = conn.get_down_part();
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.webrtc_remote_ice(dest_addr, req).await
    }

    async fn webrtc_restart_ice(&self, ctx: &Ctx, req: WebrtcRestartIceRequest) -> Option<WebrtcRestartIceResponse> {
        log::info!("On webrtc_restart_ice from other gateway");
        let conn: ClusterConnId = req.conn.parse().ok()?;
        let (dest, _session) = conn.get_down_part();
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.webrtc_restart_ice(dest_addr, req).await
    }
}

//TODO test
