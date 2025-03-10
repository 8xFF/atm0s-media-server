//!
//! This file implement forward logic from quic_rpc to worker logic
//!

use media_server_protocol::{
    endpoint::ClusterConnId,
    protobuf::{
        cluster_gateway::{
            MediaEdgeServiceHandler, RtpEngineCreateAnswerRequest, RtpEngineCreateAnswerResponse, RtpEngineCreateOfferRequest, RtpEngineCreateOfferResponse, RtpEngineDeleteRequest,
            RtpEngineDeleteResponse, RtpEngineSetAnswerRequest, RtpEngineSetAnswerResponse, WebrtcConnectRequest, WebrtcConnectResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse,
            WebrtcRestartIceRequest, WebrtcRestartIceResponse, WhepCloseRequest, WhepCloseResponse, WhepConnectRequest, WhepConnectResponse, WhepRemoteIceRequest, WhepRemoteIceResponse,
            WhipCloseRequest, WhipCloseResponse, WhipConnectRequest, WhipConnectResponse, WhipRemoteIceRequest, WhipRemoteIceResponse,
        },
        gateway::RemoteIceRequest,
    },
    transport::{
        rtpengine::{self, RtpSetAnswerRequest},
        webrtc,
        whep::{self, WhepDeleteReq, WhepRemoteIceReq},
        whip::{self, WhipDeleteReq, WhipRemoteIceReq},
        RpcReq, RpcRes,
    },
};

use crate::rpc::Rpc;

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
            RpcRes::Whip(whip::RpcRes::RemoteIce(res)) => res.ok().map(|_r| WhipRemoteIceResponse { conn }),
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
            RpcRes::Whip(whip::RpcRes::Delete(res)) => res.ok().map(|_r| WhipCloseResponse { conn }),
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
            RpcRes::Whep(whep::RpcRes::RemoteIce(res)) => res.ok().map(|_r| WhepRemoteIceResponse { conn }),
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
            RpcRes::Whep(whep::RpcRes::Delete(res)) => res.ok().map(|_r| WhepCloseResponse { conn }),
            _ => None,
        }
    }

    /* Start of sdk */
    async fn webrtc_connect(&self, ctx: &Ctx, req: WebrtcConnectRequest) -> Option<WebrtcConnectResponse> {
        log::info!("On webrtc_connect from gateway");
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::Connect(
            req.app.into(),
            req.session_id,
            req.ip.parse().ok()?,
            req.user_agent,
            req.req?,
            req.extra_data,
            req.record,
        )));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::Connect(res)) => res.ok().map(|(conn, mut r)| {
                r.conn_id = conn.to_string();
                WebrtcConnectResponse { res: Some(r) }
            }),
            _ => None,
        }
    }

    async fn webrtc_remote_ice(&self, ctx: &Ctx, req: WebrtcRemoteIceRequest) -> Option<WebrtcRemoteIceResponse> {
        log::info!("On webrtc_remote_ice from gateway");
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RemoteIce(req.conn.parse().ok()?, RemoteIceRequest { candidates: req.candidates })));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::RemoteIce(res)) => res.ok().map(|r| WebrtcRemoteIceResponse { added: r.added }),
            _ => None,
        }
    }

    async fn webrtc_restart_ice(&self, ctx: &Ctx, req: WebrtcRestartIceRequest) -> Option<WebrtcRestartIceResponse> {
        log::info!("On webrtc_restart_ice from gateway");
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RestartIce(
            req.conn.parse().ok()?,
            req.app.into(),
            req.ip.parse().ok()?,
            req.user_agent,
            req.req?,
            req.extra_data,
            req.record,
        )));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::RestartIce(res)) => res.ok().map(|(conn, mut r)| {
                r.conn_id = conn.to_string();
                WebrtcRestartIceResponse { res: Some(r) }
            }),
            _ => None,
        }
    }

    /* Start of rtp-engine */
    async fn rtp_engine_create_offer(&self, ctx: &Ctx, req: RtpEngineCreateOfferRequest) -> Option<RtpEngineCreateOfferResponse> {
        let req = req.try_into().ok()?;
        log::info!("On rtp_engine_create_offer from gateway");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::CreateOffer(req)));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateOffer(res)) => res.ok().map(|(conn, sdp)| RtpEngineCreateOfferResponse { sdp, conn: conn.to_string() }),
            _ => None,
        }
    }

    async fn rtp_engine_set_answer(&self, ctx: &Ctx, req: RtpEngineSetAnswerRequest) -> Option<RtpEngineSetAnswerResponse> {
        let conn_id = req.conn.parse().ok()?;
        log::info!("On rtp_engine_set_answer from gateway");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::SetAnswer(conn_id, RtpSetAnswerRequest { sdp: req.sdp })));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::SetAnswer(res)) => res.ok().map(|conn| RtpEngineSetAnswerResponse { conn: conn.to_string() }),
            _ => None,
        }
    }

    async fn rtp_engine_create_answer(&self, ctx: &Ctx, req: RtpEngineCreateAnswerRequest) -> Option<RtpEngineCreateAnswerResponse> {
        let req = req.try_into().ok()?;
        log::info!("On rtp_engine_create_answer from gateway");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::CreateAnswer(req)));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateAnswer(res)) => res.ok().map(|(conn, sdp)| RtpEngineCreateAnswerResponse { sdp, conn: conn.to_string() }),
            _ => None,
        }
    }

    async fn rtp_engine_delete(&self, ctx: &Ctx, req: RtpEngineDeleteRequest) -> Option<RtpEngineDeleteResponse> {
        log::info!("On rtp_engine_delete from gateway");
        let conn_id = req.conn.parse().ok()?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::Delete(conn_id)));
        ctx.req_tx.send(req).await.ok()?;
        let res = rx.await.ok()?;
        //TODO process with ICE restart
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::Delete(res)) => res.ok().map(|_r| RtpEngineDeleteResponse { conn }),
            _ => None,
        }
    }
}
