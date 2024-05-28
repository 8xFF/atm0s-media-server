use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use atm0s_sdn::NodeId;
use media_server_gateway::ServiceKind;
use media_server_protocol::{
    endpoint::ClusterConnId,
    gateway::GATEWAY_RPC_PORT,
    protobuf::{
        cluster_gateway::MediaEdgeServiceClient,
        gateway::{ConnectRequest, ConnectResponse, RemoteIceRequest, RemoteIceResponse},
    },
    rpc::{
        node_vnet_addr,
        quinn::{QuinnClient, QuinnStream},
    },
    transport::{
        webrtc,
        whep::{self, WhepConnectReq, WhepConnectRes, WhepDeleteReq, WhepDeleteRes, WhepRemoteIceReq, WhepRemoteIceRes},
        whip::{self, WhipConnectReq, WhipConnectRes, WhipDeleteReq, WhipDeleteRes, WhipRemoteIceReq, WhipRemoteIceRes},
        RpcError, RpcReq, RpcRes, RpcResult,
    },
};

use crate::errors::MediaServerError;

use super::{dest_selector::GatewayDestSelector, ip_location::Ip2Location};

pub struct MediaLocalRpcHandler {
    selector: GatewayDestSelector,
    client: MediaEdgeServiceClient<SocketAddr, QuinnClient, QuinnStream>,
    ip2location: Arc<Ip2Location>,
}

impl MediaLocalRpcHandler {
    pub fn new(selector: GatewayDestSelector, client: MediaEdgeServiceClient<SocketAddr, QuinnClient, QuinnStream>, ip2location: Arc<Ip2Location>) -> Self {
        Self { selector, client, ip2location }
    }

    pub async fn process_req(&self, conn_part: Option<(NodeId, u64)>, param: RpcReq<ClusterConnId>) -> RpcRes<ClusterConnId> {
        match param {
            RpcReq::Whip(param) => match param {
                whip::RpcReq::Connect(param) => RpcRes::Whip(whip::RpcRes::Connect(self.whip_connect(param).await)),
                whip::RpcReq::RemoteIce(param) => RpcRes::Whip(whip::RpcRes::RemoteIce(self.whip_remote_ice(conn_part, param).await)),
                whip::RpcReq::Delete(param) => RpcRes::Whip(whip::RpcRes::Delete(self.whip_delete(conn_part, param).await)),
            },
            RpcReq::Whep(param) => match param {
                whep::RpcReq::Connect(param) => RpcRes::Whep(whep::RpcRes::Connect(self.whep_connect(param).await)),
                whep::RpcReq::RemoteIce(param) => RpcRes::Whep(whep::RpcRes::RemoteIce(self.whep_remote_ice(conn_part, param).await)),
                whep::RpcReq::Delete(param) => RpcRes::Whep(whep::RpcRes::Delete(self.whep_delete(conn_part, param).await)),
            },
            RpcReq::Webrtc(param) => match param {
                webrtc::RpcReq::Connect(ip, user_agent, param) => RpcRes::Webrtc(webrtc::RpcRes::Connect(self.webrtc_connect(ip, user_agent, param).await)),
                webrtc::RpcReq::RemoteIce(conn, param) => RpcRes::Webrtc(webrtc::RpcRes::RemoteIce(self.webrtc_remote_ice(conn_part, conn, param).await)),
                webrtc::RpcReq::RestartIce(conn, ip, user_agent, req) => RpcRes::Webrtc(webrtc::RpcRes::RestartIce(self.webrtc_restart_ice(conn_part, conn, ip, user_agent, req).await)),
                webrtc::RpcReq::Delete(_) => {
                    //TODO implement delete webrtc conn
                    RpcRes::Webrtc(webrtc::RpcRes::RestartIce(Err(RpcError::new2(MediaServerError::NotImplemented))))
                }
            },
        }
    }

    /*
        Whip part
    */

    async fn whip_connect(&self, param: WhipConnectReq) -> RpcResult<WhipConnectRes<ClusterConnId>> {
        if let Some(selected) = self.selector.select(ServiceKind::Webrtc, self.ip2location.get_location(&param.ip)).await {
            let sock_addr = node_vnet_addr(selected, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {selected}");
            let rpc_req = param.into();
            let res = self.client.whip_connect(sock_addr, rpc_req).await;
            log::info!("[Gateway] response from node {selected} => {:?}", res);
            if let Some(res) = res {
                Ok(whip::WhipConnectRes {
                    sdp: res.sdp,
                    conn_id: res.conn.parse().unwrap(),
                })
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn whip_remote_ice(&self, conn_part: Option<(NodeId, u64)>, param: WhipRemoteIceReq<ClusterConnId>) -> RpcResult<WhipRemoteIceRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhipRemoteIceRequest {
                conn: param.conn_id.to_string(),
                ice: param.ice,
            };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whip_remote_ice(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whip::WhipRemoteIceRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    async fn whip_delete(&self, conn_part: Option<(NodeId, u64)>, param: WhipDeleteReq<ClusterConnId>) -> RpcResult<WhipDeleteRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhipCloseRequest { conn: param.conn_id.to_string() };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whip_close(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whip::WhipDeleteRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    /*
        Whep part
    */

    async fn whep_connect(&self, param: WhepConnectReq) -> RpcResult<WhepConnectRes<ClusterConnId>> {
        if let Some(selected) = self.selector.select(ServiceKind::Webrtc, self.ip2location.get_location(&param.ip)).await {
            let sock_addr = node_vnet_addr(selected, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {selected}");
            let rpc_req = param.into();
            let res = self.client.whep_connect(sock_addr, rpc_req).await;
            log::info!("[Gateway] response from node {selected} => {:?}", res);
            if let Some(res) = res {
                Ok(whep::WhepConnectRes {
                    sdp: res.sdp,
                    conn_id: res.conn.parse().unwrap(),
                })
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn whep_remote_ice(&self, conn_part: Option<(NodeId, u64)>, param: WhepRemoteIceReq<ClusterConnId>) -> RpcResult<WhepRemoteIceRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhepRemoteIceRequest {
                conn: param.conn_id.to_string(),
                ice: param.ice,
            };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whep_remote_ice(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whep::WhepRemoteIceRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    async fn whep_delete(&self, conn_part: Option<(NodeId, u64)>, param: WhepDeleteReq<ClusterConnId>) -> RpcResult<WhepDeleteRes> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhepCloseRequest { conn: param.conn_id.to_string() };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.whep_close(sock_addr, rpc_req).await;
            if let Some(_res) = res {
                Ok(whep::WhepDeleteRes {})
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    /*
    Webrtc part
    */

    async fn webrtc_connect(&self, ip: IpAddr, user_agent: String, req: ConnectRequest) -> RpcResult<(ClusterConnId, ConnectResponse)> {
        if let Some(selected) = self.selector.select(ServiceKind::Webrtc, self.ip2location.get_location(&ip)).await {
            let sock_addr = node_vnet_addr(selected, GATEWAY_RPC_PORT);
            log::info!("[Gateway] selected node {selected}");
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WebrtcConnectRequest {
                user_agent,
                ip: ip.to_string(),
                req: Some(req),
            };
            let res = self.client.webrtc_connect(sock_addr, rpc_req).await;
            log::info!("[Gateway] response from node {selected} => {:?}", res);
            if let Some(res) = res {
                if let Some(res) = res.res {
                    if let Ok(conn) = res.conn_id.parse() {
                        Ok((conn, res))
                    } else {
                        Err(RpcError::new2(MediaServerError::MediaResError))
                    }
                } else {
                    Err(RpcError::new2(MediaServerError::GatewayRpcError))
                }
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::NodePoolEmpty))
        }
    }

    async fn webrtc_remote_ice(&self, conn_part: Option<(NodeId, u64)>, conn: ClusterConnId, param: RemoteIceRequest) -> RpcResult<RemoteIceResponse> {
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WebrtcRemoteIceRequest {
                conn: conn.to_string(),
                candidates: param.candidates,
            };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.webrtc_remote_ice(sock_addr, rpc_req).await;
            if let Some(res) = res {
                Ok(RemoteIceResponse { added: res.added })
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }

    async fn webrtc_restart_ice(&self, conn_part: Option<(NodeId, u64)>, conn: ClusterConnId, ip: IpAddr, user_agent: String, req: ConnectRequest) -> RpcResult<(ClusterConnId, ConnectResponse)> {
        //TODO how to handle media-node down?
        if let Some((node, _session)) = conn_part {
            let rpc_req = media_server_protocol::protobuf::cluster_gateway::WebrtcRestartIceRequest {
                conn: conn.to_string(),
                ip: ip.to_string(),
                user_agent,
                req: Some(req),
            };
            log::info!("[Gateway] selected node {node}");
            let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
            let res = self.client.webrtc_restart_ice(sock_addr, rpc_req).await;
            if let Some(res) = res {
                if let Some(res) = res.res {
                    Ok((res.conn_id.parse().unwrap(), res))
                } else {
                    Err(RpcError::new2(MediaServerError::MediaResError))
                }
            } else {
                Err(RpcError::new2(MediaServerError::GatewayRpcError))
            }
        } else {
            Err(RpcError::new2(MediaServerError::InvalidConnId))
        }
    }
}

//TODO test
