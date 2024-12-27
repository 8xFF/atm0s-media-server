//!
//! This file implement forward logic from quic_rpc to worker logic
//!

use anyhow::{anyhow, Result};
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
    async fn whip_connect(&self, ctx: &Ctx, req: WhipConnectRequest) -> Result<WhipConnectResponse> {
        let req = req.try_into().map_err(|_| anyhow!("cannot convert req"))?;
        log::info!("On whip_connect from gateway");
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Connect(req)));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::Whip(whip::RpcRes::Connect(res)) => res
                .map(|r| WhipConnectResponse {
                    sdp: r.sdp,
                    conn: r.conn_id.to_string(),
                })
                .map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn whip_remote_ice(&self, ctx: &Ctx, req: WhipRemoteIceRequest) -> Result<WhipRemoteIceResponse> {
        log::info!("On whip_remote_ice from gateway");
        let conn_id = req.conn.parse().map_err(|e| anyhow!("{e}"))?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::RemoteIce(WhipRemoteIceReq { conn_id, ice: req.ice })));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whip(whip::RpcRes::RemoteIce(res)) => res.map(|_r| WhipRemoteIceResponse { conn }).map_err(|e| anyhow!("{e}")),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn whip_close(&self, ctx: &Ctx, req: WhipCloseRequest) -> Result<WhipCloseResponse> {
        log::info!("On whip_close from gateway");
        let conn_id = req.conn.parse().map_err(|e| anyhow!("{e}"))?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Delete(WhipDeleteReq { conn_id })));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whip(whip::RpcRes::Delete(res)) => res.map(|_r| WhipCloseResponse { conn }).map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    /* Start of whep */
    async fn whep_connect(&self, ctx: &Ctx, req: WhepConnectRequest) -> Result<WhepConnectResponse> {
        let req = req.try_into().map_err(|_| anyhow!("convert req error"))?;
        log::info!("On whep_connect from gateway");
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Connect(req)));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::Whep(whep::RpcRes::Connect(res)) => res
                .map(|r| WhepConnectResponse {
                    sdp: r.sdp,
                    conn: r.conn_id.to_string(),
                })
                .map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn whep_remote_ice(&self, ctx: &Ctx, req: WhepRemoteIceRequest) -> Result<WhepRemoteIceResponse> {
        log::info!("On whep_remote_ice from gateway");
        let conn_id = req.conn.parse().map_err(|e| anyhow!("{e}"))?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::RemoteIce(WhepRemoteIceReq { conn_id, ice: req.ice })));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whep(whep::RpcRes::RemoteIce(res)) => res.map(|_r| WhepRemoteIceResponse { conn }).map_err(|e| anyhow!("{e}")),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn whep_close(&self, ctx: &Ctx, req: WhepCloseRequest) -> Result<WhepCloseResponse> {
        log::info!("On whep_close from gateway");
        let conn_id = req.conn.parse().map_err(|e| anyhow!("{e}"))?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Delete(WhepDeleteReq { conn_id })));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whep(whep::RpcRes::Delete(res)) => res.map(|_r| WhepCloseResponse { conn }).map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    /* Start of sdk */
    async fn webrtc_connect(&self, ctx: &Ctx, req: WebrtcConnectRequest) -> Result<WebrtcConnectResponse> {
        log::info!("On webrtc_connect from gateway");
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::Connect(
            req.app.into(),
            req.session_id,
            req.ip.parse()?,
            req.user_agent,
            req.req.ok_or(anyhow!("Invalid request"))?,
            req.extra_data,
            req.record,
        )));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::Connect(res)) => res
                .map(|(conn, mut r)| {
                    r.conn_id = conn.to_string();
                    WebrtcConnectResponse { res: Some(r) }
                })
                .map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn webrtc_remote_ice(&self, ctx: &Ctx, req: WebrtcRemoteIceRequest) -> Result<WebrtcRemoteIceResponse> {
        log::info!("On webrtc_remote_ice from gateway");
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RemoteIce(
            req.conn.parse().map_err(|e| anyhow!("{e}"))?,
            RemoteIceRequest { candidates: req.candidates },
        )));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::RemoteIce(res)) => res.map(|r| WebrtcRemoteIceResponse { added: r.added }).map_err(|e| anyhow!("{e}")),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn webrtc_restart_ice(&self, ctx: &Ctx, req: WebrtcRestartIceRequest) -> Result<WebrtcRestartIceResponse> {
        log::info!("On webrtc_restart_ice from gateway");
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RestartIce(
            req.conn.parse().map_err(|e| anyhow!("{e}"))?,
            req.app.into(),
            req.ip.parse().map_err(|e| anyhow!("{e}"))?,
            req.user_agent,
            req.req.ok_or(anyhow!("Invalid request"))?,
            req.extra_data,
            req.record,
        )));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::RestartIce(res)) => res
                .map(|(conn, mut r)| {
                    r.conn_id = conn.to_string();
                    WebrtcRestartIceResponse { res: Some(r) }
                })
                .map_err(|e| anyhow!("{e}")),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    /* Start of rtp-engine */
    async fn rtp_engine_create_offer(&self, ctx: &Ctx, req: RtpEngineCreateOfferRequest) -> Result<RtpEngineCreateOfferResponse> {
        let req = req.try_into().map_err(|_| anyhow!("cannot convert req"))?;
        log::info!("On rtp_engine_create_offer from gateway");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::CreateOffer(req)));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateOffer(res)) => res.map(|(conn, sdp)| RtpEngineCreateOfferResponse { sdp, conn: conn.to_string() }).map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn rtp_engine_set_answer(&self, ctx: &Ctx, req: RtpEngineSetAnswerRequest) -> Result<RtpEngineSetAnswerResponse> {
        let conn_id = req.conn.parse().map_err(|e| anyhow!("{e}"))?;
        log::info!("On rtp_engine_set_answer from gateway");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::SetAnswer(conn_id, RtpSetAnswerRequest { sdp: req.sdp })));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::SetAnswer(res)) => res.map(|conn| RtpEngineSetAnswerResponse { conn: conn.to_string() }).map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn rtp_engine_create_answer(&self, ctx: &Ctx, req: RtpEngineCreateAnswerRequest) -> Result<RtpEngineCreateAnswerResponse> {
        let req = req.try_into().map_err(|_| anyhow!("cannot convert req"))?;
        log::info!("On rtp_engine_create_answer from gateway");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::CreateAnswer(req)));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateAnswer(res)) => res.map(|(conn, sdp)| RtpEngineCreateAnswerResponse { sdp, conn: conn.to_string() }).map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }

    async fn rtp_engine_delete(&self, ctx: &Ctx, req: RtpEngineDeleteRequest) -> Result<RtpEngineDeleteResponse> {
        log::info!("On rtp_engine_delete from gateway");
        let conn_id = req.conn.parse().map_err(|e| anyhow!("{e}"))?;
        let conn = req.conn.clone();
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::Delete(conn_id)));
        ctx.req_tx.send(req).await?;
        let res = rx.await?;
        //TODO process with ICE restart
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::Delete(res)) => res.map(|_r| RtpEngineDeleteResponse { conn }).map_err(|e| anyhow!(e)),
            _ => Err(anyhow!("Invalid response")),
        }
    }
}
