use std::sync::Arc;

use media_server_protocol::{
    cluster::gen_cluster_session_id,
    endpoint::ClusterConnId,
    tokens::WhipToken,
    transport::{
        whip::{self, WhipConnectReq, WhipDeleteReq, WhipRemoteIceReq},
        RpcReq, RpcRes, RpcResult,
    },
};
use media_server_secure::MediaEdgeSecure;
use poem::{http::StatusCode, Result};
use poem_openapi::{
    param::Path,
    payload::{PlainText, Response as HttpResponse},
    OpenApi,
};

use crate::rpc::Rpc;

use super::super::utils::{ApplicationSdp, ApplicationSdpPatch, CustomHttpResponse, RemoteIpAddr, TokenAuthorization, UserAgent};

pub struct WhipApis<S> {
    sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    secure: Arc<S>,
}

#[OpenApi]
impl<S: 'static + MediaEdgeSecure + Send + Sync> WhipApis<S> {
    pub fn new(sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>, secure: Arc<S>) -> Self {
        Self { sender, secure }
    }

    /// connect whip endpoint
    #[oai(path = "/endpoint", method = "post")]
    async fn whip_create(
        &self,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        let session_id = gen_cluster_session_id();
        let token = self.secure.decode_obj::<WhipToken>("whip", &token.token).ok_or(poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] create whip endpoint with token {:?}, ip {}, user_agent {}", token, ip_addr, user_agent);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Connect(WhipConnectReq {
            session_id,
            app: token.app.unwrap_or_default().into(),
            ip: ip_addr,
            sdp: body.0,
            room: token.room.into(),
            peer: token.peer.into(),
            user_agent,
            record: token.record,
            extra_data: token.extra_data,
        })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Whip(whip::RpcRes::Connect(res)) => match res {
                RpcResult::Ok(res) => {
                    log::info!("[MediaAPIs] Whip endpoint created with conn_id {}", res.conn_id);
                    Ok(CustomHttpResponse {
                        code: StatusCode::CREATED,
                        res: ApplicationSdp(res.sdp),
                        headers: vec![("location", format!("/whip/conn/{}", res.conn_id))],
                    })
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Whip endpoint creation failed with {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// patch whip conn for trickle-ice
    #[oai(path = "/conn/:conn_id", method = "patch")]
    async fn conn_whip_patch(&self, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] patch whip endpoint with sdp {}", body.0);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::RemoteIce(WhipRemoteIceReq { conn_id, ice: body.0 })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whip(whip::RpcRes::RemoteIce(res)) => match res {
                RpcResult::Ok(_res) => {
                    log::info!("[MediaAPIs] Whip endpoint patch trickle-ice with conn_id {conn_id}");
                    Ok(HttpResponse::new(ApplicationSdpPatch("".to_string())).status(StatusCode::NO_CONTENT))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Whip endpoint patch trickle-ice failed with error {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// delete whip conn
    #[oai(path = "/conn/:conn_id", method = "delete")]
    async fn conn_whip_delete(&self, conn_id: Path<String>) -> Result<PlainText<String>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] close whip endpoint conn {}", conn_id);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Delete(WhipDeleteReq { conn_id })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Whip(whip::RpcRes::Delete(res)) => match res {
                RpcResult::Ok(_res) => {
                    log::info!("[MediaAPIs] Whip endpoint closed with conn_id {conn_id}");
                    Ok(PlainText("OK".to_string()))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Whip endpoint close request failed with error {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }
}
