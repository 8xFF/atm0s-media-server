use media_server_protocol::{
    endpoint::ClusterConnId,
    protobuf::gateway::{ConnectRequest, ConnectResponse, RemoteIceRequest, RemoteIceResponse},
    transport::{
        webrtc,
        whep::{self, WhepConnectReq, WhepDeleteReq, WhepRemoteIceReq},
        whip::{self, WhipConnectReq, WhipDeleteReq, WhipRemoteIceReq},
        RpcReq, RpcRes, RpcResult,
    },
};
use poem::{http::StatusCode, web::Data, Result};
use poem_openapi::{
    auth::Bearer,
    param::Path,
    payload::{Json, PlainText, Response as HttpResponse},
    OpenApi, SecurityScheme,
};

use super::{
    utils::{ApplicationSdp, ApplicationSdpPatch, CustomHttpResponse, Protobuf, RemoteIpAddr, UserAgent},
    Rpc,
};

#[derive(SecurityScheme)]
#[oai(rename = "Token Authorization", ty = "bearer", key_in = "header", key_name = "Authorization")]
pub struct TokenAuthorization(pub Bearer);

#[derive(Clone)]
pub struct MediaServerCtx {
    pub(crate) sender: tokio::sync::mpsc::Sender<Rpc<RpcReq<ClusterConnId>, RpcRes<ClusterConnId>>>,
}

pub struct MediaApis;

#[OpenApi]
impl MediaApis {
    /// connect whip endpoint
    #[oai(path = "/whip/endpoint", method = "post")]
    async fn whip_create(
        &self,
        Data(data): Data<&MediaServerCtx>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        log::info!("[MediaAPIs] create whip endpoint with token {}, ip {}, user_agent {}", token.token, ip_addr, user_agent);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Connect(WhipConnectReq {
            ip: ip_addr,
            sdp: body.0,
            token: token.token,
            user_agent,
        })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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
    #[oai(path = "/whip/conn/:conn_id", method = "patch")]
    async fn conn_whip_patch(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] patch whip endpoint with sdp {}", body.0);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::RemoteIce(WhipRemoteIceReq { conn_id, ice: body.0 })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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

    /// post whip conn for action
    #[oai(path = "/api/whip/conn/:conn_id", method = "post")]
    async fn conn_whip_post(&self, _ctx: Data<&MediaServerCtx>, _conn_id: Path<String>, _body: Json<String>) -> Result<ApplicationSdp<String>> {
        // let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        Err(poem::Error::from_string("Not supported", StatusCode::BAD_REQUEST))
    }

    /// delete whip conn
    #[oai(path = "/whip/conn/:conn_id", method = "delete")]
    async fn conn_whip_delete(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>) -> Result<PlainText<String>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] close whip endpoint conn {}", conn_id);
        let (req, rx) = Rpc::new(RpcReq::Whip(whip::RpcReq::Delete(WhipDeleteReq { conn_id })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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

    /// connect whep endpoint
    #[oai(path = "/whep/endpoint", method = "post")]
    async fn whep_create(
        &self,
        Data(data): Data<&MediaServerCtx>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        body: ApplicationSdp<String>,
    ) -> Result<CustomHttpResponse<ApplicationSdp<String>>> {
        log::info!("[MediaAPIs] create whep endpoint with token {}, ip {}, user_agent {}", token.token, ip_addr, user_agent);
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Connect(WhepConnectReq {
            ip: ip_addr,
            sdp: body.0,
            token: token.token,
            user_agent,
        })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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
    #[oai(path = "/whep/conn/:conn_id", method = "patch")]
    async fn conn_whep_patch(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>, body: ApplicationSdpPatch<String>) -> Result<HttpResponse<ApplicationSdpPatch<String>>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] patch whep endpoint with sdp {}", body.0);
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::RemoteIce(WhepRemoteIceReq { conn_id, ice: body.0 })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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

    /// post whep conn for action
    #[oai(path = "/api/whep/conn/:conn_id", method = "post")]
    async fn conn_whep_post(&self, _ctx: Data<&MediaServerCtx>, _conn_id: Path<String>, _body: Json<String>) -> Result<ApplicationSdp<String>> {
        // let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        Err(poem::Error::from_string("Not supported", StatusCode::BAD_REQUEST))
    }

    /// delete whep conn
    #[oai(path = "/whep/conn/:conn_id", method = "delete")]
    async fn conn_whep_delete(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>) -> Result<PlainText<String>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] close whep endpoint conn {}", conn_id);
        let (req, rx) = Rpc::new(RpcReq::Whep(whep::RpcReq::Delete(WhepDeleteReq { conn_id })));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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

    /// connect webrtc
    #[oai(path = "/webrtc/connect", method = "post")]
    async fn webrtc_connect(
        &self,
        Data(data): Data<&MediaServerCtx>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        connect: Protobuf<ConnectRequest>,
    ) -> Result<HttpResponse<Protobuf<ConnectResponse>>> {
        log::info!("[MediaAPIs] create webrtc with token {}, ip {}, user_agent {}, request {:?}", token.token, ip_addr, user_agent, connect);
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::Connect(ip_addr, token.token, user_agent, connect.0)));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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
    #[oai(path = "/webrtc/:conn_id/ice-candidate", method = "post")]
    async fn webrtc_ice_candidate(&self, Data(data): Data<&MediaServerCtx>, conn_id: Path<String>, body: Protobuf<RemoteIceRequest>) -> Result<HttpResponse<Protobuf<RemoteIceResponse>>> {
        let conn_id = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[MediaAPIs] on remote ice from webrtc conn {conn_id} with ice candidate {:?}", body.0);
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RemoteIce(conn_id, body.0)));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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
    #[oai(path = "/webrtc/:conn_id/restart-ice", method = "post")]
    async fn webrtc_restart_ice(
        &self,
        Data(data): Data<&MediaServerCtx>,
        UserAgent(user_agent): UserAgent,
        RemoteIpAddr(ip_addr): RemoteIpAddr,
        TokenAuthorization(token): TokenAuthorization,
        conn_id: Path<String>,
        connect: Protobuf<ConnectRequest>,
    ) -> Result<HttpResponse<Protobuf<ConnectResponse>>> {
        let conn_id2 = conn_id.0.parse().map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!(
            "[MediaAPIs] restart_ice webrtc with token {}, ip {}, user_agent {}, conn {}, request {:?}",
            token.token,
            ip_addr,
            user_agent,
            conn_id.0,
            connect
        );
        let (req, rx) = Rpc::new(RpcReq::Webrtc(webrtc::RpcReq::RestartIce(conn_id2, ip_addr, token.token, user_agent, connect.0)));
        data.sender.send(req).await.map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
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
