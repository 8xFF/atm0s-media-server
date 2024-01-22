use std::time::{Duration, Instant};

use async_std::prelude::FutureExt;
use cluster::rpc::{
    webrtc::{WebrtcConnectRequestSender, WebrtcPatchRequest, WebrtcPatchResponse, WebrtcRemoteIceRequest, WebrtcRemoteIceResponse},
    RpcReqRes,
};
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut, RpcResponse},
    EndpointRpcIn, EndpointRpcOut,
};

use str0m::{bwe::Bitrate, change::SdpOffer, media::KeyframeRequestKind, net::Receive, rtp::ExtensionValues, Candidate, Input, Output, Rtc, RtcError};
use transport::{RequestKeyframeKind, Transport, TransportError, TransportIncomingEvent, TransportOutgoingEvent};

use self::{
    internal::{
        rpc::{TransportRpcIn, TransportRpcOut, UpdateSdpResponse},
        Str0mAction, WebrtcTransportInternal,
    },
    life_cycle::TransportLifeCycle,
    net::ComposeSocket,
    rtp_packet_convert::MediaPacketConvert,
    sdp_box::{SdpBox, SdpBoxRewriteScope},
    str0m_event_convert::Str0mEventConvert,
};

pub(crate) mod internal;
pub mod life_cycle;
mod mid_convert;
mod mid_history;
mod net;
mod pt_mapping;
mod rtp_packet_convert;
pub mod sdp_box;
mod str0m_event_convert;

const INIT_BWE_BITRATE_KBPS: u64 = 1500; //1500kbps

pub enum WebrtcTransportEvent {
    RemoteIce(Box<dyn RpcReqRes<WebrtcRemoteIceRequest, WebrtcRemoteIceResponse>>),
    SdpPatch(Box<dyn RpcReqRes<WebrtcPatchRequest, WebrtcPatchResponse>>),
}

pub struct WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    socket: ComposeSocket,
    sdp_box: Option<SdpBox>,
    rtc: Rtc,
    internal: WebrtcTransportInternal<L>,
    buf: Vec<u8>,
    event_convert: Str0mEventConvert,
    pkt_convert: MediaPacketConvert,
}

impl<L> WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    /// Create new transport with provided life_cycle. In current version we have 3 types:
    ///   - SDK
    ///   - Whip
    ///   - Whep
    ///
    /// Next param is sdp_rewrite_scope, which is use to determine if we need to rewrite sdp or not.
    /// If rewrite has 2 types: SdpBoxRewriteScope::TrackOnly and SdpBoxRewriteScope::StreamAndTrack
    pub async fn new(life_cycle: L, sdp_rewrite: Option<SdpBoxRewriteScope>) -> Result<Self, std::io::Error> {
        let mut rtc = Rtc::builder()
            .enable_bwe(Some(Bitrate::kbps(INIT_BWE_BITRATE_KBPS)))
            .set_ice_lite(false)
            .set_rtp_mode(true)
            .set_stats_interval(Some(Duration::from_millis(500)))
            .build();
        rtc.direct_api().enable_twcc_feedback();

        let socket = ComposeSocket::new(0).await?;

        for (addr, proto) in socket.local_addrs() {
            log::info!("[TransportWebrtc] listen on {}::/{}", proto, addr);
            let candidate = Candidate::host(addr, proto).expect("Should create candidate");
            rtc.add_local_candidate(candidate);
        }

        log::info!("[TransportWebrtc] created");

        Ok(Self {
            socket,
            sdp_box: sdp_rewrite.map(|scope| SdpBox { scope }),
            rtc,
            internal: WebrtcTransportInternal::new(life_cycle),
            buf: vec![0; 2000],
            event_convert: Default::default(),
            pkt_convert: Default::default(),
        })
    }

    pub fn map_remote_stream(&mut self, sender: WebrtcConnectRequestSender) {
        self.internal.map_remote_stream(sender);
    }

    pub fn on_remote_sdp(&mut self, sdp: &str) -> Result<String, RtcError> {
        let sdp_offer = SdpOffer::from_sdp_string(sdp)?;
        let sdp_answer = self.rtc.sdp_api().accept_offer(sdp_offer)?;

        //sync payload_type mapping
        self.event_convert.str0m_sync_codec_config(self.rtc.codec_config());
        self.pkt_convert.str0m_sync_codec_config(self.rtc.codec_config());

        if let Some(sdp_box) = &mut self.sdp_box {
            Ok(sdp_box.rewrite_answer(&sdp_answer.to_sdp_string()))
        } else {
            Ok(sdp_answer.to_string())
        }
    }

    fn pop_internal_str0m_actions(&mut self, now_ms: u64) {
        while let Some(action) = self.internal.str0m_action() {
            match action {
                Str0mAction::Rpc(rpc) => match rpc {
                    TransportRpcIn::UpdateSdp(req) => {
                        if let Ok(sdp_offer) = SdpOffer::from_sdp_string(&req.data.sdp) {
                            for sender in req.data.senders {
                                self.internal.map_remote_stream(sender);
                            }
                            match self.rtc.sdp_api().accept_offer(sdp_offer) {
                                Ok(sdp_answer) => {
                                    let sdp = if let Some(sdp_box) = &mut self.sdp_box {
                                        sdp_box.rewrite_answer(&sdp_answer.to_sdp_string())
                                    } else {
                                        sdp_answer.to_sdp_string()
                                    };
                                    let res = RpcResponse::success(req.req_id, UpdateSdpResponse { sdp });
                                    self.internal.on_transport_rpc(now_ms, TransportRpcOut::UpdateSdpRes(res));
                                }
                                Err(e) => {
                                    log::error!("[TransportWebrtc] error on accept offer {:?}", e);
                                    let res = RpcResponse::error(req.req_id);
                                    self.internal.on_transport_rpc(now_ms, TransportRpcOut::UpdateSdpRes(res));
                                }
                            }
                        }
                    }
                },
                Str0mAction::Media(track_id, seq_no, mut pkt) => {
                    let mid = self.event_convert.mid_for_track(track_id).cloned().expect("Should has mid");
                    if let Some(stream) = self.rtc.direct_api().stream_tx_by_mid(mid, None) {
                        self.pkt_convert.rewrite_codec(&mut pkt);
                        stream
                            .write_rtp(
                                self.pkt_convert.to_pt(&pkt),
                                seq_no,
                                pkt.time,
                                Instant::now(),
                                pkt.marker,
                                ExtensionValues::default(),
                                pkt.nackable,
                                pkt.payload,
                            )
                            .expect("Should ok");
                    } else {
                        log::warn!("[TransportWebrtc] missing track for mid {}", mid);
                        debug_assert!(false, "should not missing mid");
                    }
                }
                Str0mAction::RequestKeyFrame(track_id, kind) => {
                    let mid = self.event_convert.mid_for_track(track_id).cloned().expect("Should has mid");
                    if let Some(stream) = self.rtc.direct_api().stream_rx_by_mid(mid, None) {
                        match kind {
                            RequestKeyframeKind::Pli => stream.request_keyframe(KeyframeRequestKind::Pli),
                            RequestKeyframeKind::Fir => stream.request_keyframe(KeyframeRequestKind::Fir),
                        }
                    } else {
                        log::warn!("[TransportWebrtc] missing track for mid {} when requesting key-frame", mid);
                        debug_assert!(false, "should not missing mid");
                    }
                }
                Str0mAction::Datachannel(cid, msg) => {
                    if let Some(cid) = self.event_convert.channel_id(cid) {
                        if let Some(mut channel) = self.rtc.channel(cid) {
                            if let Err(e) = channel.write(false, msg.as_bytes()) {
                                log::error!("[TransportWebrtc] write datachannel error {:?}", e);
                            }
                        } else {
                            log::warn!("[TransportWebrtc] missing channel for id {:?}", cid);
                            debug_assert!(false, "should not missing channel id");
                        }
                    } else {
                        log::warn!("[TransportWebrtc] missing channel for id {:?}", cid);
                        debug_assert!(false, "should not missing channel id");
                    }
                }
                Str0mAction::ConfigEgressBitrate { current, desired } => {
                    let mut bwe = self.rtc.bwe();
                    bwe.set_current_bitrate(Bitrate::bps(current as u64));
                    bwe.set_desired_bitrate(Bitrate::bps(desired as u64));
                }
                Str0mAction::LimitIngressBitrate { track_id, max } => {
                    let mid = self.event_convert.mid_for_track(track_id).cloned().expect("Should has mid");
                    if let Some(stream) = self.rtc.direct_api().stream_rx_by_mid(mid, None) {
                        log::debug!("[TransportWebrtc] on limit ingress bitrate mid {} max {}", mid, max);
                        stream.request_remb(Bitrate::bps(max as u64));
                    } else {
                        log::warn!("[TransportWebrtc] missing track for mid {} when requesting REMB {}", mid, max);
                    }
                }
                Str0mAction::RemoteIce(candidate) => match Candidate::from_sdp_string(&candidate) {
                    Ok(can) => {
                        log::info!("on remote ice {:?}", can);
                        self.rtc.add_remote_candidate(can);
                    }
                    Err(e) => {
                        log::error!("error on parse ice candidate {:?}", e);
                    }
                },
                Str0mAction::Close => {
                    if let Some(cid) = self.event_convert.channel_id(0) {
                        log::info!("[TransportWebrtc] close request");
                        self.rtc.direct_api().close_data_channel(cid);
                        self.rtc.disconnect();
                    }
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl<L> Transport<WebrtcTransportEvent, EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn, EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut> for WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.internal.on_tick(now_ms)
    }

    fn on_event(&mut self, now_ms: u64, event: TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>) -> Result<(), TransportError> {
        self.internal.on_endpoint_event(now_ms, event)?;
        self.pop_internal_str0m_actions(now_ms);
        Ok(())
    }

    fn on_custom_event(&mut self, now_ms: u64, event: WebrtcTransportEvent) -> Result<(), TransportError> {
        self.internal.on_custom_event(now_ms, event)?;
        self.pop_internal_str0m_actions(now_ms);
        Ok(())
    }

    async fn recv(&mut self, now_ms: u64) -> Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError> {
        if let Some(action) = self.internal.endpoint_action() {
            return action;
        }
        self.pop_internal_str0m_actions(now_ms);

        let timeout = match self.rtc.poll_output() {
            Ok(o) => match o {
                Output::Timeout(t) => t,
                Output::Transmit(t) => {
                    if let Err(_e) = self.socket.send_to(&t.contents, t.proto, t.source, t.destination).await {
                        log::debug!("Error sending data {} => {}: {}", t.source, t.destination, _e);
                    }
                    return Ok(TransportIncomingEvent::Continue);
                }
                Output::Event(e) => {
                    if let Ok(Some(e)) = self.event_convert.str0m_to_internal(e) {
                        self.internal.on_str0m_event(now_ms, e)?;
                        if let Some(action) = self.internal.endpoint_action() {
                            return action;
                        } else {
                            return Ok(TransportIncomingEvent::Continue);
                        }
                    } else {
                        return Ok(TransportIncomingEvent::Continue);
                    }
                }
            },
            Err(e) => {
                log::error!("[TransportWebrtc] error polling rtc: {:?}", e);
                todo!("process this error");
            }
        };

        // Duration until timeout.
        let duration = timeout - Instant::now();

        // socket.set_read_timeout(Some(0)) is not ok
        if duration.is_zero() {
            // Drive time forwards in rtc straight away.
            return match self.rtc.handle_input(Input::Timeout(Instant::now())) {
                Ok(_) => Ok(TransportIncomingEvent::Continue),
                Err(e) => {
                    log::error!("[TransportWebrtc] error handle input rtc: {:?}", e);
                    Ok(TransportIncomingEvent::Continue)
                }
            };
        }

        // Scale up buffer to receive an entire UDP packet.
        unsafe {
            self.buf.set_len(2000);
        }

        // Try to receive. Because we have a timeout on the socket,
        // we will either receive a packet, or timeout.
        // This is where having an async loop shines. We can await multiple things to
        // happen such as outgoing media data, the timeout and incoming network traffic.
        // When using async there is no need to set timeout on the socket.
        let input = match self.socket.recv(&mut self.buf).timeout(duration).await {
            Ok(Ok((n, source, destination, proto))) => {
                // UDP data received.
                unsafe {
                    self.buf.set_len(n);
                }
                log::trace!("received from {} => {}, proto {} len {}", source, destination, proto, n);
                Input::Receive(
                    Instant::now(),
                    Receive {
                        proto,
                        source,
                        destination,
                        contents: self.buf.as_slice().try_into().unwrap(),
                    },
                )
            }
            Ok(Err(e)) => {
                log::error!("[TransportWebrtc] network error {:?}", e);
                return Err(TransportError::NetworkError);
            }
            Err(_e) => {
                // Expected error for set_read_timeout().
                // One for windows, one for the rest.
                Input::Timeout(Instant::now())
            }
        };

        // Input is either a Timeout or Receive of data. Both drive the state forward.
        if let Err(e) = self.rtc.handle_input(input) {
            log::error!("[TransportWebrtc] error handle input rtc: {:?}", e);
            todo!("handle rtc error")
        }
        return Ok(TransportIncomingEvent::Continue);
    }

    async fn close(&mut self, _now_ms: u64) {
        self.internal.close();
    }
}

impl<L: TransportLifeCycle> Drop for WebrtcTransport<L> {
    fn drop(&mut self) {
        log::info!("[TransportWebrtc] drop");
    }
}
