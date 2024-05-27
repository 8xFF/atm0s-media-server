use media_server_protocol::{
    endpoint::ClusterConnId,
    protobuf::cluster_gateway::{
        MediaEdgeServiceHandler, WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse, WebrtcRestartIceRequest, WebrtcRestartIceResponse, WhipCloseRequest,
        WhipCloseResponse, WhipConnectRequest, WhipConnectResponse, WhipRemoteIceRequest, WhipRemoteIceResponse,
    },
    transport::{
        whip::{self, WhipConnectReq, WhipDeleteReq, WhipRemoteIceReq},
        RpcReq, RpcRes,
    },
};

use crate::http::Rpc;

#[derive(Clone)]
pub struct Ctx {
    pub(crate) req_tx: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
}

#[derive(Default)]
pub struct MediaRpcHandlerImpl {}

impl MediaEdgeServiceHandler<Ctx> for MediaRpcHandlerImpl {
    async fn whip_connect(&self, ctx: &Ctx, req: WhipConnectRequest) -> Option<WhipConnectResponse> {
        log::info!("On whip_connect from gateway");
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Connect(WhipConnectReq {
            ip: req.ip_addr.parse().unwrap(),
            sdp: req.sdp,
            room: req.room.into(),
            peer: req.peer.into(),
            user_agent: req.user_agent,
        })));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::Whip(whip::RpcRes::Connect(res)) => res.ok().map(|r| WhipConnectResponse {
                sdp: r.sdp,
                conn: r.conn_id.to_string(),
            }),
            _ => None,
        }
    }

    async fn whip_remote_ice(&self, ctx: &Ctx, req: WhipRemoteIceRequest) -> Option<WhipRemoteIceResponse> {
        log::info!("On whip_remote_ice from gateway");
        let conn_id = req.conn.parse().ok()?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::RemoteIce(WhipRemoteIceReq { conn_id, ice: req.ice })));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whip(whip::RpcRes::RemoteIce(res)) => res.ok().map(|r| WhipRemoteIceResponse { conn }),
            _ => None,
        }
    }

    async fn whip_close(&self, ctx: &Ctx, req: WhipCloseRequest) -> Option<WhipCloseResponse> {
        log::info!("On whip_close from gateway");
        let conn_id = req.conn.parse().ok()?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Delete(WhipDeleteReq { conn_id })));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whip(whip::RpcRes::Delete(res)) => res.ok().map(|r| WhipCloseResponse { conn }),
            _ => None,
        }
    }

    async fn webrtc_connec(&self, ctx: &Ctx, req: WebrtcConnectRequest) -> Option<WebrtcConnectResponse> {
        todo!()
    }

    async fn webrtc_remote_ice(&self, ctx: &Ctx, req: WebrtcRemoteIceRequest) -> Option<WebrtcRemoteIceResponse> {
        todo!()
    }

    async fn webrtc_restart_ice(&self, ctx: &Ctx, req: WebrtcRestartIceRequest) -> Option<WebrtcRestartIceResponse> {
        todo!()
    }
}
