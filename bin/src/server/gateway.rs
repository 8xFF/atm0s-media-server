use std::{sync::Arc, time::Duration};

use atm0s_sdn::{features::FeaturesEvent, secure::StaticKeyAuthorization, services::visualization, SdnBuilder, SdnControllerUtils, SdnExtOut, SdnOwner};
use clap::Parser;
use media_server_gateway::{store_service::GatewayStoreServiceBuilder, ServiceKind, STORE_SERVICE_ID};
use media_server_protocol::{
    gateway::{generate_gateway_zone_tag, GATEWAY_RPC_PORT},
    protobuf::cluster_gateway::{MediaEdgeServiceClient, MediaEdgeServiceServer},
    rpc::{
        node_vnet_addr,
        quinn::{QuinnClient, QuinnServer},
    },
    transport::{whep, whip, RpcError, RpcReq, RpcRes},
};
use media_server_secure::jwt::{MediaEdgeSecureJwt, MediaGatewaySecureJwt};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

use crate::{
    errors::MediaServerError,
    http::run_gateway_http_server,
    quinn::{make_quinn_client, make_quinn_server, VirtualNetwork},
    NodeConfig,
};
use sans_io_runtime::{backend::PollingBackend, ErrorDebugger, ErrorDebugger2};

use self::dest_selector::build_dest_selector;

mod dest_selector;
mod rpc_handler;

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SC {
    Visual(visualization::Control),
    Gateway(media_server_gateway::store_service::Control),
}

#[derive(Clone, Debug, convert_enum::From, convert_enum::TryInto)]
enum SE {
    Visual(visualization::Event),
    Gateway(media_server_gateway::store_service::Event),
}
type TC = ();
type TW = ();

#[derive(Debug, Parser)]
pub struct Args {
    /// Location latude
    #[arg(env, long, default_value_t = 0.0)]
    lat: f32,

    /// Location longtude
    #[arg(env, long, default_value_t = 0.0)]
    lon: f32,
}

pub async fn run_media_gateway(workers: usize, http_port: Option<u16>, node: NodeConfig, args: Args) {
    rustls::crypto::ring::default_provider().install_default().expect("should install ring as default");

    let default_cluster_cert_buf = include_bytes!("../../certs/cluster.cert");
    let default_cluster_key_buf = include_bytes!("../../certs/cluster.key");
    let default_cluster_cert = CertificateDer::from(default_cluster_cert_buf.to_vec());
    let default_cluster_key = PrivatePkcs8KeyDer::from(default_cluster_key_buf.to_vec());

    let edge_secure = Arc::new(MediaEdgeSecureJwt::from(node.secret.as_bytes()));
    let gateway_secure = Arc::new(MediaGatewaySecureJwt::from(node.secret.as_bytes()));
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel(1024);
    if let Some(http_port) = http_port {
        tokio::spawn(async move {
            if let Err(e) = run_gateway_http_server(http_port, req_tx, edge_secure, gateway_secure).await {
                log::error!("HTTP Error: {}", e);
            }
        });
    }

    let node_id = node.node_id;

    let mut builder = SdnBuilder::<(), SC, SE, TC, TW>::new(node_id, node.udp_port, node.custom_addrs);

    builder.set_authorization(StaticKeyAuthorization::new(&node.secret));
    builder.set_manual_discovery(vec!["gateway".to_string(), generate_gateway_zone_tag(node.zone)], vec!["gateway".to_string()]);
    builder.add_service(Arc::new(GatewayStoreServiceBuilder::new(node.zone, args.lat, args.lon)));

    for seed in node.seeds {
        builder.add_seed(seed);
    }

    let mut controller = builder.build::<PollingBackend<SdnOwner, 128, 128>>(workers);
    let (selector, mut requester) = build_dest_selector();

    //
    // Vnet is a virtual udp layer for creating RPC handlers, we separate media server to 2 layer
    // - async for business logic like proxy, logging handling
    // - sync with sans-io style for media data
    //
    let (mut vnet, vnet_tx, mut vnet_rx) = VirtualNetwork::new(node.node_id);
    let media_rpc_socket = vnet.udp_socket(GATEWAY_RPC_PORT).await.expect("Should open virtual port for gateway rpc");
    let mut media_rpc_server = MediaEdgeServiceServer::new(
        QuinnServer::new(make_quinn_server(media_rpc_socket, default_cluster_key, default_cluster_cert.clone()).expect("Should create endpoint for media rpc server")),
        rpc_handler::Ctx {},
        rpc_handler::MediaRpcHandlerImpl::default(),
    );
    let media_rpc_socket = vnet.udp_socket(0).await.expect("Should open virtual port for gateway rpc");
    let media_rpc_client = MediaEdgeServiceClient::new(QuinnClient::new(make_quinn_client(media_rpc_socket, &vec![]).expect("Should create endpoint for media rpc client")));

    tokio::task::spawn_local(async move {
        media_rpc_server.run().await;
    });

    tokio::task::spawn_local(async move { while let Some(_) = vnet.recv().await {} });

    loop {
        if controller.process().is_none() {
            break;
        }
        while let Ok(control) = vnet_rx.try_recv() {
            controller.feature_control((), control.into());
        }
        while let Some(out) = requester.recv() {
            controller.service_control(STORE_SERVICE_ID.into(), (), out.into());
        }
        while let Ok(req) = req_rx.try_recv() {
            let res_tx = req.answer_tx;
            let param = req.req;
            let conn_part = param.get_conn_part();
            let selector = selector.clone();
            let client = media_rpc_client.clone();
            tokio::spawn(async move {
                match param {
                    RpcReq::Whip(param) => match param {
                        whip::RpcReq::Connect(param) => {
                            //TODO get lat and lon
                            if let Some(selected) = selector.select(ServiceKind::Webrtc, 1.0, 1.0).await {
                                let sock_addr = node_vnet_addr(selected, GATEWAY_RPC_PORT);
                                log::info!("[Gateway] selected node {selected}");
                                let rpc_req = param.into();
                                let res = client.whip_connect(sock_addr, rpc_req).await;
                                log::info!("[Gateway] response from node {selected} => {:?}", res);
                                if let Some(res) = res {
                                    res_tx
                                        .send(RpcRes::Whip(whip::RpcRes::Connect(Ok(whip::WhipConnectRes {
                                            sdp: res.sdp,
                                            conn_id: res.conn.parse().unwrap(),
                                        }))))
                                        .print_err2("answer http request error");
                                } else {
                                    res_tx
                                        .send(RpcRes::Whip(whip::RpcRes::Connect(Err(RpcError::new2(MediaServerError::GatewayRpcError)))))
                                        .print_err2("answer http request error");
                                }
                            }
                        }
                        whip::RpcReq::RemoteIce(req) => {
                            if let Some((node, _session)) = conn_part {
                                let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhipRemoteIceRequest {
                                    conn: req.conn_id.to_string(),
                                    ice: req.ice,
                                };
                                log::info!("[Gateway] selected node {node}");
                                let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
                                let res = client.whip_remote_ice(sock_addr, rpc_req).await;
                                if let Some(_res) = res {
                                    res_tx
                                        .send(RpcRes::Whip(whip::RpcRes::RemoteIce(Ok(whip::WhipRemoteIceRes {}))))
                                        .print_err2("answer http request error");
                                } else {
                                    res_tx
                                        .send(RpcRes::Whip(whip::RpcRes::RemoteIce(Err(RpcError::new2(MediaServerError::GatewayRpcError)))))
                                        .print_err2("answer http request error");
                                }
                            } else {
                                res_tx
                                    .send(RpcRes::Whip(whip::RpcRes::RemoteIce(Err(RpcError::new2(MediaServerError::InvalidConnId)))))
                                    .print_err2("answer http request error");
                            }
                        }
                        whip::RpcReq::Delete(req) => {
                            if let Some((node, _session)) = conn_part {
                                let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhipCloseRequest { conn: req.conn_id.to_string() };
                                log::info!("[Gateway] selected node {node}");
                                let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
                                let res = client.whip_close(sock_addr, rpc_req).await;
                                if let Some(_res) = res {
                                    res_tx.send(RpcRes::Whip(whip::RpcRes::Delete(Ok(whip::WhipDeleteRes {})))).print_err2("answer http request error");
                                } else {
                                    res_tx
                                        .send(RpcRes::Whip(whip::RpcRes::Delete(Err(RpcError::new2(MediaServerError::GatewayRpcError)))))
                                        .print_err2("answer http request error");
                                }
                            } else {
                                res_tx
                                    .send(RpcRes::Whip(whip::RpcRes::Delete(Err(RpcError::new2(MediaServerError::InvalidConnId)))))
                                    .print_err2("answer http request error");
                            }
                        }
                    },
                    RpcReq::Whep(param) => match param {
                        whep::RpcReq::Connect(param) => {
                            //TODO get lat and lon
                            if let Some(selected) = selector.select(ServiceKind::Webrtc, 1.0, 1.0).await {
                                let sock_addr = node_vnet_addr(selected, GATEWAY_RPC_PORT);
                                log::info!("[Gateway] selected node {selected}");
                                let rpc_req = param.into();
                                let res = client.whep_connect(sock_addr, rpc_req).await;
                                log::info!("[Gateway] response from node {selected} => {:?}", res);
                                if let Some(res) = res {
                                    res_tx
                                        .send(RpcRes::Whep(whep::RpcRes::Connect(Ok(whep::WhepConnectRes {
                                            sdp: res.sdp,
                                            conn_id: res.conn.parse().unwrap(),
                                        }))))
                                        .print_err2("answer http request error");
                                } else {
                                    res_tx
                                        .send(RpcRes::Whep(whep::RpcRes::Connect(Err(RpcError::new2(MediaServerError::GatewayRpcError)))))
                                        .print_err2("answer http request error");
                                }
                            }
                        }
                        whep::RpcReq::RemoteIce(req) => {
                            if let Some((node, _session)) = conn_part {
                                let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhepRemoteIceRequest {
                                    conn: req.conn_id.to_string(),
                                    ice: req.ice,
                                };
                                log::info!("[Gateway] selected node {node}");
                                let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
                                let res = client.whep_remote_ice(sock_addr, rpc_req).await;
                                if let Some(_res) = res {
                                    res_tx
                                        .send(RpcRes::Whep(whep::RpcRes::RemoteIce(Ok(whep::WhepRemoteIceRes {}))))
                                        .print_err2("answer http request error");
                                } else {
                                    res_tx
                                        .send(RpcRes::Whep(whep::RpcRes::RemoteIce(Err(RpcError::new2(MediaServerError::GatewayRpcError)))))
                                        .print_err2("answer http request error");
                                }
                            } else {
                                res_tx
                                    .send(RpcRes::Whep(whep::RpcRes::RemoteIce(Err(RpcError::new2(MediaServerError::InvalidConnId)))))
                                    .print_err2("answer http request error");
                            }
                        }
                        whep::RpcReq::Delete(req) => {
                            if let Some((node, _session)) = conn_part {
                                let rpc_req = media_server_protocol::protobuf::cluster_gateway::WhepCloseRequest { conn: req.conn_id.to_string() };
                                log::info!("[Gateway] selected node {node}");
                                let sock_addr = node_vnet_addr(node, GATEWAY_RPC_PORT);
                                let res = client.whep_close(sock_addr, rpc_req).await;
                                if let Some(_res) = res {
                                    res_tx.send(RpcRes::Whep(whep::RpcRes::Delete(Ok(whep::WhepDeleteRes {})))).print_err2("answer http request error");
                                } else {
                                    res_tx
                                        .send(RpcRes::Whep(whep::RpcRes::Delete(Err(RpcError::new2(MediaServerError::GatewayRpcError)))))
                                        .print_err2("answer http request error");
                                }
                            } else {
                                res_tx
                                    .send(RpcRes::Whep(whep::RpcRes::Delete(Err(RpcError::new2(MediaServerError::InvalidConnId)))))
                                    .print_err2("answer http request error");
                            }
                        }
                    },
                    media_server_protocol::transport::RpcReq::Webrtc(_) => todo!(),
                }
            });
        }

        while let Some(out) = controller.pop_event() {
            match out {
                SdnExtOut::ServicesEvent(_, _, SE::Gateway(event)) => {
                    requester.on_event(event);
                }
                SdnExtOut::FeaturesEvent(_, FeaturesEvent::Socket(event)) => {
                    if let Err(e) = vnet_tx.try_send(event) {
                        log::error!("[MediaEdge] forward Sdn SocketEvent error {:?}", e);
                    }
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
