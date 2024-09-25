use std::sync::Arc;

use media_server_protocol::{
    cluster::gen_cluster_session_id,
    endpoint::ClusterConnId,
    tokens::WhepToken,
    transport::{
        whep::{self, WhepConnectReq, WhepDeleteReq, WhepRemoteIceReq},
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
use rand::random;

use crate::rpc::Rpc;

use super::super::utils::{ApplicationSdp, ApplicationSdpPatch, CustomHttpResponse, RemoteIpAddr, TokenAuthorization, UserAgent};

pub struct WhepApis<S> {
    sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    secure: Arc<S>,
}

#[OpenApi]
impl<S: 'static + MediaEdgeSecure + Send + Sync> WhepApis<S> {
    pub fn new(sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>, secure: Arc<S>) -> Self {
        Self { sender, secure }
    }

    /// connect whep endpoint
    #[oai(path = "/endpoint", method = "post")]
    async fn whep_create(
        &self,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        let session_id = gen_cluster_session_id();
        let token = self.secure.decode_obj::<WhepToken>("whep", &token.token).ok_or(poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] create whep endpoint with token {:?}, ip {}, user_agent {}", token, ip_addr, user_agent);
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Connect(WhepConnectReq {
            session_id,
            app: token.app.unwrap_or_default().into(),
            ip: ip_addr,
            sdp: body.0,
            room: token.room.into(),
            peer: token.peer.unwrap_or_else(|| format!("whep-{}", (random::<u64>()))).into(),
            user_agent,
            extra_data: token.extra_data,
        })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Whep(whep::RpcRes::Connect(res)) => match res {
                RpcResult::Ok(res) => {
                    log::info!("[MediaAPIs] Whep endpoint created with conn_id {}", res.conn_id);
                    Ok(CustomHttpResponse {
                        code: StatusCode::CREATED,
                        res: ApplicationSdp(res.sdp),
                        headers: vec![("location", format!("/whep/conn/{}", res.conn_id))],
                    })
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Whep endpoint creation failed with {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// patch whep conn for trickle-ice
    #[oai(path = "/conn/:conn_id", method = "patch")]
    async fn conn_whep_patch(&self, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] patch whep endpoint with sdp {}", body.0);
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::RemoteIce(WhepRemoteIceReq { conn_id, ice: body.0 })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        //TODO process with ICE restart
        match res {
            RpcRes::Whep(whep::RpcRes::RemoteIce(res)) => match res {
                RpcResult::Ok(_res) => {
                    log::info!("[MediaAPIs] Whep endpoint patch trickle-ice with conn_id {conn_id}");
                    Ok(HttpResponse::new(ApplicationSdpPatch("".to_string())).status(StatusCode::NO_CONTENT))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Whep endpoint patch trickle-ice failed with error {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// delete whep conn
    #[oai(path = "/conn/:conn_id", method = "delete")]
    async fn conn_whep_delete(&self, conn_id: Path<String>) -> Result<PlainText<String>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] close whep endpoint conn {}", conn_id);
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Delete(WhepDeleteReq { conn_id })));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Whep(whep::RpcRes::Delete(res)) => match res {
                RpcResult::Ok(_res) => {
                    log::info!("[MediaAPIs] Whep endpoint closed with conn_id {conn_id}");
                    Ok(PlainText("OK".to_string()))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] Whep endpoint close request failed with error {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }
}
