use std::net::SocketAddr;

use media_server_gateway::ServiceKind;
use media_server_protocol::{
    endpoint::ClusterConnId,
    gateway::GATEWAY_RPC_PORT,
    protobuf::cluster_gateway::{
        MediaEdgeServiceClient, MediaEdgeServiceHandler, WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse, WebrtcRestartIceRequest,
        WebrtcRestartIceResponse, WhepCloseRequest, WhepCloseResponse, WhepConnectRequest, WhepConnectResponse, WhepRemoteIceRequest, WhepRemoteIceResponse, WhipCloseRequest, WhipCloseResponse,
        WhipConnectRequest, WhipConnectResponse, WhipRemoteIceRequest, WhipRemoteIceResponse,
    },
    rpc::{
        node_vnet_addr,
        quinn::{QuinnClient, QuinnStream},
    },
    transport::ConnLayer,
};

use super::dest_selector::GatewayDestSelector;

#[derive(Clone)]
pub struct Ctx {
    pub(crate) selector: GatewayDestSelector,
    pub(crate) client: MediaEdgeServiceClient<SocketAddr, QuinnClient, QuinnStream>,
}

#[derive(Default)]
pub struct MediaRpcHandlerImpl {}

impl MediaEdgeServiceHandler<Ctx> for MediaRpcHandlerImpl {
    async fn whip_connect(&self, ctx: &Ctx, req: WhipConnectRequest) -> Option<WhipConnectResponse> {
        log::info!("On whip_connect from other gateway");
        //TODO detect location
        let dest = ctx.selector.select(ServiceKind::Webrtc, 1.1, 1.1).await?;
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.whip_connect(dest_addr, req).await
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
        log::info!("On whep_connect from other gateway");
        //TODO detect location
        let dest = ctx.selector.select(ServiceKind::Webrtc, 1.1, 1.1).await?;
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.whep_connect(dest_addr, req).await
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
        log::info!("On webrtc_connect from other gateway");
        //TODO detect location
        let dest = ctx.selector.select(ServiceKind::Webrtc, 1.1, 1.1).await?;
        let dest_addr = node_vnet_addr(dest, GATEWAY_RPC_PORT);
        ctx.client.webrtc_connect(dest_addr, req).await
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
