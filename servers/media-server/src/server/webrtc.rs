use std::{ops::Deref, time::Duration};

use async_std::{channel::Sender, prelude::FutureExt as _};
use clap::Parser;
use cluster::{
    rpc::{
        gateway::{NodeHealthcheckResponse, NodePing, NodePong, ServiceInfo},
        general::{MediaEndpointCloseResponse, NodeInfo, ServerType},
        webrtc::{WebrtcConnectRequestSender, WebrtcConnectResponse, WebrtcPatchRequest, WebrtcPatchResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse},
        whep::WhepConnectResponse,
        whip::WhipConnectResponse,
        RpcEmitter, RpcEndpoint, RpcReqRes, RpcRequest, RPC_NODE_PING,
    },
    Cluster, ClusterEndpoint, EndpointSubscribeScope, MixMinusAudioMode, RemoteBitrateControlMode, VerifyObject, INNER_GATEWAY_SERVICE, MEDIA_SERVER_SERVICE,
};
use endpoint::BitrateLimiterType;
use futures::{select, FutureExt};
use media_utils::{AutoCancelTask, ErrorDebugger, StringCompression, SystemTimer, Timer};
use metrics_dashboard::build_dashboard_route;
use poem::{web::Json, Route};
use poem_openapi::OpenApiService;
use transport_webrtc::{SdkTransportLifeCycle, SdpBoxRewriteScope, WhepTransportLifeCycle, WhipTransportLifeCycle};

use crate::{rpc::http::HttpRpcServer, server::webrtc::session::run_webrtc_endpoint};

#[cfg(feature = "embed-samples")]
use crate::rpc::http::EmbeddedFilesEndpoint;
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
pub struct WebrtcArgs {
    /// Max conn
    #[arg(env, long, default_value_t = 100)]
    pub max_conn: u64,
}

pub async fn run_webrtc_server<C, CR, RPC, REQ, EMITTER>(
    http_port: u16,
    http_tls: bool,
    _opts: WebrtcArgs,
    ctx: MediaServerContext<InternalControl>,
    mut cluster: C,
    rpc_endpoint: RPC,
) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    let timer = SystemTimer();
    let mut rpc_endpoint = WebrtcClusterRpc::new(rpc_endpoint);
    let mut http_server: HttpRpcServer<RpcEvent> = crate::rpc::http::HttpRpcServer::new(http_port, http_tls);
    let node_info = NodeInfo {
        node_id: cluster.node_id(),
        address: format!("{}", cluster.node_addr()),
        server_type: ServerType::WEBRTC,
    };
    let api_service = OpenApiService::new(WebrtcHttpApis, "Webrtc Server", "1.0.0").server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    #[cfg(feature = "embed-samples")]
    let samples = EmbeddedFilesEndpoint::<Files>::new(Some("index.html".to_string()));
    #[cfg(not(feature = "embed-samples"))]
    let samples = StaticFilesEndpoint::new("./servers/media/public/webrtc").show_files_listing().index_file("index.html");
    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/node-info/", poem::endpoint::make_sync(move |_| Json(node_info.clone())))
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()))
        .nest("/samples", samples);

    // Init media-server related metrics
    ctx.init_metrics();

    http_server.start(route, ctx.clone()).await;
    let mut whep_counter = 0;

    let node_id = cluster.node_id();
    let rpc_emitter = rpc_endpoint.emitter();
    let ctx_c = ctx.clone();
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
                            usage: ((ctx_c.conns_live() * 100) / ctx_c.conns_max()) as u8,
                            live: ctx_c.conns_live() as u32,
                            max: ctx_c.conns_max() as u32,
                            addr: None,
                            domain: None,
                        }),
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
                let s_token = if let Some(s_token) = req.param().verify(ctx.verifier().deref()) {
                    s_token
                } else {
                    req.answer(Err("INVALID_TOKEN"));
                    continue;
                };
                let room = s_token.room;
                let peer = s_token.peer.unwrap_or("publisher".to_string());
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
                    &peer,
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
                    MixMinusAudioMode::Disabled,
                    0,
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
                let s_token = if let Some(s_token) = req.param().verify(ctx.verifier().deref()) {
                    s_token
                } else {
                    req.answer(Err("INVALID_TOKEN"));
                    continue;
                };
                let room = s_token.room;
                let peer = s_token.peer.unwrap_or_else(|| format!("whep-{}", whep_counter));
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
                    MixMinusAudioMode::Disabled,
                    0,
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
                if req.param().verify(ctx.verifier().deref()).is_none() {
                    req.answer(Err("INVALID_TOKEN"));
                    continue;
                };
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
                    param.mix_minus_audio.unwrap_or(MixMinusAudioMode::Disabled),
                    3,
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
                if let Some(old_tx) = ctx.get_conn(&req.param().conn_id) {
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
