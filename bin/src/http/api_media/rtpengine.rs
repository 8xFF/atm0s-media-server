use std::sync::Arc;

use media_server_protocol::{
    cluster::gen_cluster_session_id,
    endpoint::ClusterConnId,
    tokens::RtpEngineToken,
    transport::{
        rtpengine::{self, RtpCreateAnswerRequest, RtpCreateOfferRequest, RtpSetAnswerRequest},
        RpcReq, RpcRes, RpcResult,
    },
};
use media_server_secure::MediaEdgeSecure;
use poem::{http::StatusCode, web::Path, Result};
use poem_openapi::{payload::PlainText, OpenApi};

use crate::{
    http::utils::{ApplicationSdp, CustomHttpResponse},
    rpc::Rpc,
};

use super::super::utils::{RemoteIpAddr, TokenAuthorization};

pub struct RtpengineApis<S> {
    sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    secure: Arc<S>,
}

#[OpenApi]
impl<S: 'static + MediaEdgeSecure + Send + Sync> RtpengineApis<S> {
    pub fn new(sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>, secure: Arc<S>) -> Self {
        Self { sender, secure }
    }

    /// connect rtpengine endpoint with offer
    #[oai(path = "/offer", method = "post")]
    async fn create_offer(&self, RemoteIpAddr(ip_addr): RemoteIpAddr, TokenAuthorization(token): TokenAuthorization) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        let session_id = gen_cluster_session_id();
        let (app_ctx, token) = self.secure.decode_token::<RtpEngineToken>(&token.token).ok_or(poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] create rtpengine endpoint with token {token:?}, ip {ip_addr}");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::CreateOffer(RtpCreateOfferRequest {
            app: app_ctx,
            session_id,
            room: token.room.into(),
            peer: token.peer.into(),
            record: token.record,
            extra_data: token.extra_data,
        })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateOffer(res)) => match res {
                RpcResult::Ok((conn, sdp)) => {
                    log::info!("[MediaAPIs] Rtpengine endpoint created with conn_id {conn}");
                    Ok(CustomHttpResponse {
                        code: StatusCode::CREATED,
                        res: ApplicationSdp(sdp),
                        headers: vec![("location", format!("/rtpengine/conn/{}", conn))],
                    })
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Rtpengine endpoint creation failed with {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// create rtpengine endpoint with answer
    #[oai(path = "/answer", method = "post")]
    async fn create_answer(
        &self,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        let session_id = gen_cluster_session_id();
        let (app_ctx, token) = self.secure.decode_token::<RtpEngineToken>(&token.token).ok_or(poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] create rtpengine endpoint with token {token:?}, ip {ip_addr}");
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::CreateAnswer(RtpCreateAnswerRequest {
            app: app_ctx,
            session_id,
            sdp: body.0,
            room: token.room.into(),
            peer: token.peer.into(),
            record: token.record,
            extra_data: token.extra_data,
        })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::CreateAnswer(res)) => match res {
                RpcResult::Ok((conn, sdp)) => {
                    log::info!("[MediaAPIs] Rtpengine endpoint created with conn_id {conn}");
                    Ok(CustomHttpResponse {
                        code: StatusCode::CREATED,
                        res: ApplicationSdp(sdp),
                        headers: vec![("location", format!("/rtpengine/conn/{}", conn))],
                    })
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Rtpengine endpoint creation failed with {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// patch rtpengine conn for trickle-ice
    #[oai(path = "/conn/:conn_id", method = "patch")]
    async fn conn_whep_patch(&self, conn_id: Path<String>, body: ApplicationSdp<String>) -> Result<PlainText<String>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] patch rtpengine endpoint with remote sdp {}", body.0);
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::SetAnswer(conn_id, RtpSetAnswerRequest { sdp: body.0 })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::SetAnswer(res)) => match res {
                RpcResult::Ok(_res) => {
                    log::info!("[MediaAPIs] Rtpengine endpoint patched answer sdp with conn_id {conn_id}");
                    Ok(PlainText("OK".to_string()))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Rtpengine endpoint patch answer sdp failed with error {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// delete rtpengine conn
    #[oai(path = "/conn/:conn_id", method = "delete")]
    async fn conn_whep_delete(&self, conn_id: Path<String>) -> Result<PlainText<String>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] close rtpengine endpoint conn {}", conn_id);
        let (req, rx) = Rpc::new(RpcReq::RtpEngine(rtpengine::RpcReq::Delete(conn_id)));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::RtpEngine(rtpengine::RpcRes::Delete(res)) => match res {
                RpcResult::Ok(_res) => {
                    log::info!("[MediaAPIs] Rtpengine endpoint closed with conn_id {conn_id}");
                    Ok(PlainText("OK".to_string()))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Rtpengine endpoint close request failed with error {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }
}
