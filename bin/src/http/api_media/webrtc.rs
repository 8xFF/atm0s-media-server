use std::sync::Arc;

use media_server_protocol::{
    cluster::gen_cluster_session_id,
    endpoint::ClusterConnId,
    protobuf::gateway::{ConnectRequest, ConnectResponse, RemoteIceRequest, RemoteIceResponse},
    tokens::WebrtcToken,
    transport::{webrtc, RpcReq, RpcRes, RpcResult},
};
use media_server_secure::MediaEdgeSecure;
use poem::{http::StatusCode, Result};
use poem_openapi::{param::Path, payload::Response as HttpResponse, OpenApi};

use crate::rpc::Rpc;

use super::super::utils::{Protobuf, RemoteIpAddr, TokenAuthorization, UserAgent};

pub struct WebrtcApis<S> {
    sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
    secure: Arc<S>,
}

#[OpenApi]
impl<S: 'static + MediaEdgeSecure + Send + Sync> WebrtcApis<S> {
    pub fn new(sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>, secure: Arc<S>) -> Self {
        Self { sender, secure }
    }

    /// connect webrtc
    #[oai(path = "/connect", method = "post")]
    async fn webrtc_connect(
        &self,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        connect: Protobuf<ConnectRequest>,
    ) -> Result<HttpResponse<Protobuf<ConnectResponse>>> {
        let session_id = gen_cluster_session_id();
        let token = self.secure.decode_obj::<WebrtcToken>("webrtc", &token.token).ok_or(poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] create webrtc with token {:?}, ip {}, user_agent {}, request {:?}", token, ip_addr, user_agent, connect);
        if let Some(join) = &connect.join {
            if token.room != Some(join.room.clone()) {
                return Err(poem::Error::from_string("Wrong room".to_string(), StatusCode::FORBIDDEN));
            }

            if token.peer != Some(join.peer.clone()) {
                return Err(poem::Error::from_string("Wrong peer".to_string(), StatusCode::FORBIDDEN));
            }
        }
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::Connect(session_id, ip_addr, user_agent, connect.0, token.extra_data, token.record)));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::Connect(res)) => match res {
                RpcResult::Ok((conn, res)) => {
                    log::info!("[MediaAPIs] Webrtc endpoint created with conn_id {}", res.conn_id);
                    Ok(HttpResponse::new(Protobuf(ConnectResponse {
                        conn_id: conn.to_string(),
                        sdp: res.sdp,
                        ice_lite: res.ice_lite,
                    })))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] webrtc endpoint creation failed with {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// patch webrtc conn for trickle-ice
    #[oai(path = "/:conn_id/ice-candidate", method = "post")]
    async fn webrtc_ice_candidate(&self, conn_id: Path<String>, body: Protobuf<RemoteIceRequest>) -> Result<HttpResponse<Protobuf<RemoteIceResponse>>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] on remote ice from webrtc conn {conn_id} with ice candidate {:?}", body.0);
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RemoteIce(conn_id, body.0)));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        //TODO process with ICE restart
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::RemoteIce(res)) => match res {
                RpcResult::Ok(res) => {
                    log::info!("[MediaAPIs] webrtc endpoint trickle-ice with conn_id {conn_id}");
                    Ok(HttpResponse::new(Protobuf(res)))
                }
                RpcResult::Err(e) => {
                    log::warn!("[MediaAPIs] webrtc endpoint patch trickle-ice failed with error {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// webrtc restart ice
    #[oai(path = "/:conn_id/restart-ice", method = "post")]
    async fn webrtc_restart_ice(
        &self,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        conn_id: Path<String>,
        connect: Protobuf<ConnectRequest>,
    ) -> Result<HttpResponse<Protobuf<ConnectResponse>>> {
        let conn_id2 = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        let token = self.secure.decode_obj::<WebrtcToken>("webrtc", &token.token).ok_or(poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        if let Some(join) = &connect.join {
            if token.room != Some(join.room.clone()) {
                return Err(poem::Error::from_string("Wrong room".to_string(), StatusCode::FORBIDDEN));
            }

            if token.peer != Some(join.peer.clone()) {
                return Err(poem::Error::from_string("Wrong peer".to_string(), StatusCode::FORBIDDEN));
            }
        }
        log::info!("[MediaAPIs] restart_ice webrtc, ip {}, user_agent {}, conn {}, request {:?}", ip_addr, user_agent, conn_id.0, connect);
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RestartIce(conn_id2, ip_addr, user_agent, connect.0, token.extra_data, token.record)));
        self.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        match res {
            RpcRes::Webrtc(webrtc::RpcRes::RestartIce(res)) => match res {
                RpcResult::Ok((conn, res)) => {
                    log::info!("[MediaAPIs] Webrtc endpoint restart ice with conn_id {}", res.conn_id);
                    Ok(HttpResponse::new(Protobuf(ConnectResponse {
                        conn_id: conn.to_string(),
                        sdp: res.sdp,
                        ice_lite: res.ice_lite,
                    })))
                }
                RpcResult::Err(e) => {
                    log::warn!("Webrtc endpoint restart ice failed with {e}");
                    Err(poem::Error::from_string(e.to_string(), StatusCode::BAD_REQUEST))
                }
            },
            _ => Err(poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }
}
