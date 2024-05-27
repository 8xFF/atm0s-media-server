use media_server_protocol::{
    endpoint::ClusterConnId,
    protobuf::cluster_gateway::{
        MediaEdgeServiceHandler, WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse, WebrtcRestartIceRequest, WebrtcRestartIceResponse, WhepCloseRequest,
        WhepCloseResponse, WhepConnectRequest, WhepConnectResponse, WhepRemoteIceRequest, WhepRemoteIceResponse, WhipCloseRequest, WhipCloseResponse, WhipConnectRequest, WhipConnectResponse,
        WhipRemoteIceRequest, WhipRemoteIceResponse,
    },
    transport::{
        whep::{self, WhepDeleteReq, WhepRemoteIceReq},
        whip::{self, WhipDeleteReq, WhipRemoteIceReq},
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
    /* Start of whip */
    async fn whip_connect(&self, ctx: &Ctx, req: WhipConnectRequest) -> Option<WhipConnectResponse> {
        let req = req.try_into().ok()?;
        log::info!("On whip_connect from gateway");
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Connect(req)));
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

    /* Start of whep */
    async fn whep_connect(&self, ctx: &Ctx, req: WhepConnectRequest) -> Option<WhepConnectResponse> {
        let req = req.try_into().ok()?;
        log::info!("On whep_connect from gateway");
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Connect(req)));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::Whep(whep::RpcRes::Connect(res)) => res.ok().map(|r| WhepConnectResponse {
                sdp: r.sdp,
                conn: r.conn_id.to_string(),
            }),
            _ => None,
        }
    }

    async fn whep_remote_ice(&self, ctx: &Ctx, req: WhepRemoteIceRequest) -> Option<WhepRemoteIceResponse> {
        log::info!("On whep_remote_ice from gateway");
        let conn_id = req.conn.parse().ok()?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::RemoteIce(WhepRemoteIceReq { conn_id, ice: req.ice })));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whep(whep::RpcRes::RemoteIce(res)) => res.ok().map(|r| WhepRemoteIceResponse { conn }),
            _ => None,
        }
    }

    async fn whep_close(&self, ctx: &Ctx, req: WhepCloseRequest) -> Option<WhepCloseResponse> {
        log::info!("On whep_close from gateway");
        let conn_id = req.conn.parse().ok()?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Delete(WhepDeleteReq { conn_id })));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whep(whep::RpcRes::Delete(res)) => res.ok().map(|r| WhepCloseResponse { conn }),
            _ => None,
        }
    }

    /* Start of sdk */
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
