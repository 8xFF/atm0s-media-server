use std::{sync::Arc, time::Duration};

use async_std::stream::StreamExt;
use clap::Parser;
use cluster::{
    rpc::{
        gateway::parse_conn_id,
        general::{MediaEndpointCloseRequest, MediaEndpointCloseResponse},
        webrtc::{WebrtcConnectRequest, WebrtcConnectResponse, WebrtcPatchRequest, WebrtcPatchResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse},
        whep::{WhepConnectRequest, WhepConnectResponse},
        whip::{WhipConnectRequest, WhipConnectResponse},
        RpcEmitter, RpcEndpoint, RpcRequest, RPC_MEDIA_ENDPOINT_CLOSE, RPC_WEBRTC_CONNECT, RPC_WEBRTC_ICE, RPC_WEBRTC_PATCH, RPC_WHEP_CONNECT, RPC_WHIP_CONNECT,
    },
    Cluster, ClusterEndpoint, MEDIA_SERVER_SERVICE,
};
use futures::{select, FutureExt};
use media_utils::{SystemTimer, Timer};
use poem::Route;
use poem_openapi::OpenApiService;

use crate::rpc::http::HttpRpcServer;

#[cfg(feature = "embed-samples")]
use crate::rpc::http::EmbeddedFilesEndpoint;
#[cfg(feature = "embed-samples")]
use rust_embed::RustEmbed;

#[cfg(not(feature = "embed-samples"))]
use poem::endpoint::StaticFilesEndpoint;

#[cfg(feature = "embed-samples")]
#[derive(RustEmbed)]
#[folder = "public"]
pub struct Files;

use self::{
    logic::{GatewayLogic, ServiceType},
    rpc::{cluster::GatewayClusterRpc, http::GatewayHttpApis, RpcEvent},
};

mod logic;
mod rpc;

/// Media Server Webrtc
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct GatewayArgs {}

pub async fn run_gateway_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, _opts: GatewayArgs, _cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let mut rpc_endpoint = GatewayClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port);

    let timer = Arc::new(SystemTimer());
    let api_service = OpenApiService::new(GatewayHttpApis, "Gateway Server", "1.0.0").server("http://localhost:3000");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    #[cfg(feature = "embed-samples")]
    let samples = EmbeddedFilesEndpoint::<Files>::new(Some("index.html".to_string()));
    #[cfg(not(feature = "embed-samples"))]
    let samples = StaticFilesEndpoint::new("./servers/media-server/public/").index_file("index.html");
    let route = Route::new()
        .nest("/", api_service)
        .nest("/ui/", ui)
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
        .nest("/samples", samples);

    http_server.start(route).await;
    let mut tick = async_std::stream::interval(Duration::from_millis(100));
    let mut gateway_logic = GatewayLogic::new();
    let rpc_emitter = rpc_endpoint.emitter();

    loop {
        let rpc = select! {
            _ = tick.next().fuse() => {
                gateway_logic.on_tick(timer.now_ms());
                continue;
            }
            rpc = http_server.recv().fuse() => {
                rpc.ok_or("HTTP_SERVER_ERROR")?
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc.ok_or("CLUSTER_RPC_ERROR")?
            }
        };

        match rpc {
            RpcEvent::NodePing(req) => {
                log::info!("[Gateway] node ping {:?}", req.param());
                req.answer(Ok(gateway_logic.on_ping(timer.now_ms(), req.param())));
            }
            RpcEvent::WhipConnect(req) => {
                log::info!("[Gateway] whip connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                let mut nodes = gateway_logic.best_nodes(ServiceType::Webrtc, 60, 80, 1);
                if let Some(node_id) = nodes.pop() {
                    log::info!("[Gateway] whip connect to node {}", node_id);
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WhipConnectRequest, WhipConnectResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_WHIP_CONNECT, req.param().clone(), 5000)
                            .await;
                        log::info!("[Gateway] whip connect res from media-server {:?}", res.as_ref().map(|_| ()));
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    log::warn!("[Gateway] whip connect but media-server pool empty");
                    req.answer(Err("NODE_POOL_EMPTY"));
                }
            }
            RpcEvent::WhepConnect(req) => {
                log::info!("[Gateway] whep connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                let mut nodes = gateway_logic.best_nodes(ServiceType::Webrtc, 60, 80, 1);
                if let Some(node_id) = nodes.pop() {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WhepConnectRequest, WhepConnectResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_WHEP_CONNECT, req.param().clone(), 5000)
                            .await;
                        log::info!("[Gateway] whep connect res from media-server {:?}", res.as_ref().map(|_| ()));
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    log::warn!("[Gateway] whep connect but media-server pool empty");
                    req.answer(Err("NODE_POOL_EMPTY"));
                }
            }
            RpcEvent::WebrtcConnect(req) => {
                log::info!("[Gateway] webrtc connect compressed_sdp: {:?}", req.param().compressed_sdp.as_ref().map(|sdp| sdp.len()));
                let mut nodes = gateway_logic.best_nodes(ServiceType::Webrtc, 60, 80, 1);
                if let Some(node_id) = nodes.pop() {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WebrtcConnectRequest, WebrtcConnectResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_WEBRTC_CONNECT, req.param().clone(), 5000)
                            .await;
                        log::info!("[Gateway] webrtc connect res from media-server {:?}", res.as_ref().map(|_| ()));
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    log::warn!("[Gateway] webrtc connect but media-server pool empty");
                    req.answer(Err("NODE_POOL_EMPTY"));
                }
            }
            RpcEvent::WebrtcRemoteIce(req) => {
                if let Some((node_id, _)) = parse_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_WEBRTC_ICE, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
            RpcEvent::WebrtcSdpPatch(req) => {
                if let Some((node_id, _)) = parse_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<WebrtcPatchRequest, WebrtcPatchResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_WEBRTC_PATCH, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
            RpcEvent::MediaEndpointClose(req) => {
                if let Some((node_id, _)) = parse_conn_id(&req.param().conn_id) {
                    let rpc_emitter = rpc_emitter.clone();
                    async_std::task::spawn_local(async move {
                        let res = rpc_emitter
                            .request::<MediaEndpointCloseRequest, MediaEndpointCloseResponse>(MEDIA_SERVER_SERVICE, Some(node_id), RPC_MEDIA_ENDPOINT_CLOSE, req.param().clone(), 5000)
                            .await;
                        req.answer(res.map_err(|_e| "INTERNAL_ERROR"));
                    });
                } else {
                    req.answer(Err("WRONG_CONN_ID"));
                }
            }
        }
    }
}
