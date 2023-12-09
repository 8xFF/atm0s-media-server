use std::time::Duration;

use async_std::{channel::Sender, prelude::FutureExt as _};
use clap::Parser;
use cluster::{
    rpc::{
        gateway::{NodeHealthcheckResponse, NodePing, NodePong, ServiceInfo},
        general::MediaEndpointCloseResponse,
        webrtc::{WebrtcConnectRequestSender, WebrtcConnectResponse, WebrtcPatchRequest, WebrtcPatchResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse},
        whep::WhepConnectResponse,
        whip::WhipConnectResponse,
        RpcEmitter, RpcEndpoint, RpcReqRes, RpcRequest, RPC_NODE_PING,
    },
    Cluster, ClusterEndpoint, EndpointSubscribeScope, RemoteBitrateControlMode, INNER_GATEWAY_SERVICE,
};
use endpoint::BitrateLimiterType;
use futures::{select, FutureExt};
use media_utils::{AutoCancelTask, ErrorDebugger, StringCompression, SystemTimer, Timer};
use poem::Route;
use poem_openapi::OpenApiService;
use transport_webrtc::{SdkTransportLifeCycle, SdpBoxRewriteScope, WhepTransportLifeCycle, WhipTransportLifeCycle};

use crate::{rpc::http::HttpRpcServer, server::webrtc::session::run_webrtc_endpoint};

#[cfg(feature = "embed-samples")]
use poem::endpoint::EmbeddedFilesEndpoint;
#[cfg(feature = "embed-samples")]
use rust_embed::RustEmbed;

#[cfg(not(feature = "embed-samples"))]
use poem::endpoint::StaticFilesEndpoint;

#[cfg(feature = "embed-samples")]
#[derive(RustEmbed)]
#[folder = "public/webrtc"]
pub struct Files;

use self::rpc::{cluster::WebrtcClusterRpc, http::WebrtcHttpApis, RpcEvent};

use super::MediaServerContext;

pub enum InternalControl {
    RemoteIce(Box<dyn RpcReqRes<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>>),
    SdpPatch(Box<dyn RpcReqRes<WebrtcPatchRequest, WebrtcPatchResponse>>),
    ForceClose(Sender<()>),
}

mod rpc;
mod session;

/// Media Server Webrtc
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct WebrtcArgs {}

pub async fn run_webrtc_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, _opts: WebrtcArgs, ctx: MediaServerContext<InternalControl>, mut cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let timer = SystemTimer();
    let mut rpc_endpoint = WebrtcClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port);

    let api_service = OpenApiService::new(WebrtcHttpApis, "Webrtc Server", "1.0.0").server("http://localhost:3000");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    #[cfg(feature = "embed-samples")]
    let samples = EmbeddedFilesEndpoint::<Files>::new();
    #[cfg(not(feature = "embed-samples"))]
    let samples = StaticFilesEndpoint::new("./servers/media/public/webrtc").show_files_listing().index_file("index.html");
    let route = Route::new()
        .nest("/", api_service)
        .nest("/ui/", ui)
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
        .nest("/samples", samples);

    http_server.start(route).await;
    let mut whep_counter = 0;

    let node_id = cluster.node_id();
    let rpc_emitter = rpc_endpoint.emitter();
    let _ping_task: AutoCancelTask<_> = async_std::task::spawn_local(async move {
        async_std::task::sleep(Duration::from_secs(10)).await;
        loop {
            if let Err(e) = rpc_emitter
                .request::<_, NodePong>(
                    INNER_GATEWAY_SERVICE,
                    None,
                    RPC_NODE_PING,
                    NodePing {
                        node_id,
                        rtmp: None,
                        sip: None,
                        webrtc: Some(ServiceInfo {
                            usage: 0, //TODO implement real info
                            max: 100,
                            addr: None,
                            domain: None,
                        }),
                        token: "demo-token".to_string(), //TODO implement real-token
                    },
                    5000,
                )
                .await
            {
                log::error!("[WebrtcMediaServer] ping gateway error {:?}", e);
            } else {
                log::info!("[WebrtcMediaServer] ping gateway success");
            }
            async_std::task::sleep(Duration::from_secs(1)).await;
        }
    })
    .into();

    loop {
        let rpc = select! {
            rpc = http_server.recv().fuse() => {
                rpc.ok_or("HTTP_SERVER_ERROR")?
            },
            rpc = rpc_endpoint.recv().fuse() => {
                rpc.ok_or("CLUSTER_RPC_ERROR")?
            }
        };

        match rpc {
            RpcEvent::NodeHeathcheck(req) => {
                req.answer(Ok(NodeHealthcheckResponse { success: true }));
            }
            RpcEvent::WhipConnect(req) => {
                //TODO validate token to get room
                let room = req.param().token.as_str();
                let peer = "publisher";
                let (sdp, is_compress) = match (&req.param().sdp, &req.param().compressed_sdp) {
                    (Some(sdp), _) => (sdp.clone(), false),
                    (_, Some(compressed_sdp)) => {
                        if let Some(sdp) = StringCompression::default().uncompress(&compressed_sdp) {
                            (sdp, true)
                        } else {
                            req.answer(Err("DECOMPRESS_SDP_ERROR"));
                            continue;
                        }
                    }
                    _ => {
                        req.answer(Err("MISSING_SDP"));
                        continue;
                    }
                };
                log::info!("[MediaServer] on whip connection from {} {}", room, peer);
                let life_cycle = WhipTransportLifeCycle::new(timer.now_ms());
                match run_webrtc_endpoint(
                    ctx.clone(),
                    &mut cluster,
                    life_cycle,
                    EndpointSubscribeScope::RoomManual,
                    BitrateLimiterType::MaxBitrateOnly,
                    &room,
                    peer,
                    &sdp,
                    vec![
                        WebrtcConnectRequestSender {
                            kind: "audio".to_string(),
                            name: "audio_main".to_string(),
                            label: "audio_main".to_string(),
                            uuid: "audio_main".to_string(),
                            screen: None,
                        },
                        WebrtcConnectRequestSender {
                            kind: "video".to_string(),
                            name: "video_main".to_string(),
                            label: "video_main".to_string(),
                            uuid: "video_main".to_string(),
                            screen: None,
                        },
                    ],
                    None,
                )
                .await
                {
                    Ok((sdp, conn_id)) => {
                        if is_compress {
                            req.answer(Ok(WhipConnectResponse {
                                conn_id,
                                sdp: None,
                                compressed_sdp: Some(StringCompression::default().compress(&sdp)),
                            }));
                        } else {
                            req.answer(Ok(WhipConnectResponse {
                                conn_id,
                                sdp: Some(sdp),
                                compressed_sdp: None,
                            }));
                        }
                    }
                    Err(err) => {
                        req.answer(Err(&err.code));
                    }
                }
            }
            RpcEvent::WhepConnect(req) => {
                //TODO validate token to get room
                let room = req.param().token.as_str();
                let peer = format!("whep-{}", whep_counter);
                let (sdp, is_compress) = match (&req.param().sdp, &req.param().compressed_sdp) {
                    (Some(sdp), _) => (sdp.clone(), false),
                    (_, Some(compressed_sdp)) => {
                        if let Some(sdp) = StringCompression::default().uncompress(&compressed_sdp) {
                            (sdp, true)
                        } else {
                            req.answer(Err("DECOMPRESS_SDP_ERROR"));
                            continue;
                        }
                    }
                    _ => {
                        req.answer(Err("MISSING_SDP"));
                        continue;
                    }
                };
                whep_counter += 1;

                log::info!("[MediaServer] on whep connection from {} {}", room, peer);
                let life_cycle = WhepTransportLifeCycle::new(timer.now_ms());
                match run_webrtc_endpoint(
                    ctx.clone(),
                    &mut cluster,
                    life_cycle,
                    EndpointSubscribeScope::RoomAuto,
                    BitrateLimiterType::MaxBitrateOnly,
                    &room,
                    &peer,
                    &sdp,
                    vec![],
                    Some(SdpBoxRewriteScope::OnlyTrack),
                )
                .await
                {
                    Ok((sdp, conn_id)) => {
                        if is_compress {
                            req.answer(Ok(WhepConnectResponse {
                                conn_id,
                                sdp: None,
                                compressed_sdp: Some(StringCompression::default().compress(&sdp)),
                            }));
                        } else {
                            req.answer(Ok(WhepConnectResponse {
                                conn_id,
                                sdp: Some(sdp),
                                compressed_sdp: None,
                            }));
                        }
                    }
                    Err(err) => {
                        req.answer(Err(&err.code));
                    }
                }
            }
            RpcEvent::WebrtcConnect(req) => {
                let (sdp, is_compress) = match (&req.param().sdp, &req.param().compressed_sdp) {
                    (Some(sdp), _) => (sdp.clone(), false),
                    (_, Some(compressed_sdp)) => {
                        if let Some(sdp) = StringCompression::default().uncompress(&compressed_sdp) {
                            (sdp, true)
                        } else {
                            req.answer(Err("DECOMPRESS_SDP_ERROR"));
                            continue;
                        }
                    }
                    _ => {
                        req.answer(Err("MISSING_SDP"));
                        continue;
                    }
                };
                let param = req.param();
                log::info!("[MediaServer] on webrtc connection from {} {}", param.room, param.peer);
                let sub_scope = param.sub_scope.unwrap_or(EndpointSubscribeScope::RoomAuto);
                let life_cycle = SdkTransportLifeCycle::new(timer.now_ms());
                match run_webrtc_endpoint(
                    ctx.clone(),
                    &mut cluster,
                    life_cycle,
                    sub_scope,
                    match param.remote_bitrate_control_mode {
                        Some(RemoteBitrateControlMode::MaxBitrateOnly) => BitrateLimiterType::MaxBitrateOnly,
                        _ => BitrateLimiterType::DynamicWithConsumers,
                    },
                    &param.room,
                    &param.peer,
                    &sdp,
                    param.senders.clone(),
                    Some(SdpBoxRewriteScope::StreamAndTrack),
                )
                .await
                {
                    Ok((sdp, conn_id)) => {
                        if is_compress {
                            req.answer(Ok(WebrtcConnectResponse {
                                conn_id,
                                sdp: None,
                                compressed_sdp: Some(StringCompression::default().compress(&sdp)),
                            }));
                        } else {
                            req.answer(Ok(WebrtcConnectResponse {
                                conn_id,
                                sdp: Some(sdp),
                                compressed_sdp: None,
                            }));
                        }
                    }
                    Err(err) => {
                        req.answer(Err(&err.code));
                    }
                }
            }
            RpcEvent::WebrtcRemoteIce(req) => {
                if let Some(tx) = ctx.get_conn(&req.param().conn_id) {
                    async_std::task::spawn(async move {
                        if let Err(_e) = tx.send(InternalControl::RemoteIce(req)).await {
                            log::error!("[WebrtcServer] internal queue error")
                            //req.answer(Err("INTERNAL_QUEUE_ERROR"));
                        }
                    });
                } else {
                    req.answer(Err("CONN_NOT_FOUND"));
                }
            }
            RpcEvent::WebrtcSdpPatch(req) => {
                if let Some(tx) = ctx.get_conn(&req.param().conn_id) {
                    async_std::task::spawn(async move {
                        if let Err(_e) = tx.send(InternalControl::SdpPatch(req)).await {
                            log::error!("[WebrtcServer] internal queue error")
                            //req.answer(Err("INTERNAL_QUEUE_ERROR"));
                        }
                    });
                } else {
                    req.answer(Err("CONN_NOT_FOUND"));
                }
            }
            RpcEvent::MediaEndpointClose(req) => {
                if let Some(old_tx) = ctx.close_conn(&req.param().conn_id) {
                    async_std::task::spawn(async move {
                        let (tx, rx) = async_std::channel::bounded(1);
                        old_tx.send(InternalControl::ForceClose(tx.clone())).await.log_error("need send");
                        if let Ok(e) = rx.recv().timeout(Duration::from_secs(1)).await {
                            let control_res = e.map_err(|_e| "INTERNAL_QUEUE_ERROR");
                            req.answer(control_res.map(|_| MediaEndpointCloseResponse { success: true }));
                        } else {
                            req.answer(Err("REQUEST_TIMEOUT"));
                        }
                    });
                } else {
                    req.answer(Err("NOT_FOUND"));
                }
            }
        }
    }
}
