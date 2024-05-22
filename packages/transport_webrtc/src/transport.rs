use std::{
    net::{IpAddr, SocketAddr},
    ops::Deref,
    time::{Duration, Instant},
};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointReqId, EndpointRes},
    transport::{Transport, TransportInput, TransportOutput},
};
use media_server_protocol::{
    endpoint::{PeerId, RoomId},
    media::MediaPacket,
    protobuf::gateway::ConnectRequest,
    transport::{RpcError, RpcResult},
};
use media_server_utils::{RtpSeqExtend, Small2dMap};
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    collections::DynamicDeque,
    return_if_err, return_if_none, return_if_some, TaskSwitcherChild,
};
use str0m::{
    bwe::Bitrate,
    change::{DtlsCert, SdpOffer},
    channel::{ChannelConfig, ChannelId},
    format::CodecConfig,
    ice::IceCreds,
    media::{KeyframeRequestKind, Mid},
    net::{Protocol, Receive},
    rtp::ExtensionValues,
    Candidate, Rtc,
};

use crate::{media::LocalMediaConvert, WebrtcError};

mod bwe_state;
mod webrtc;
mod whep;
mod whip;

pub enum VariantParams {
    Whip(RoomId, PeerId),
    Whep(RoomId, PeerId),
    Webrtc(IpAddr, String, ConnectRequest),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Variant {
    Whip,
    Whep,
    Webrtc,
}

pub enum ExtIn {
    RemoteIce(u64, Variant, Vec<String>),
    RestartIce(u64, Variant, IpAddr, String, String, ConnectRequest),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtOut {
    RemoteIce(u64, Variant, RpcResult<u32>),
    /// response is (ice_lite, answer_sdp)
    RestartIce(u64, Variant, RpcResult<(bool, String)>),
}

#[derive(Debug, PartialEq, Eq)]
enum InternalRpcReq {
    SetRemoteSdp(String),
}

enum InternalRpcRes {
    SetRemoteSdp(String),
}

#[derive(Debug, PartialEq, Eq)]
enum InternalOutput {
    Str0mKeyframe(Mid, KeyframeRequestKind),
    Str0mLimitBitrate(Mid, u64),
    Str0mSendMedia(Mid, MediaPacket),
    Str0mSendData(ChannelId, Vec<u8>),
    Str0mBwe(u64, u64),
    Str0mResetBwe(u64),
    RpcReq(u32, InternalRpcReq),
    TransportOutput(TransportOutput<ExtOut>),
}

trait TransportWebrtcInternal {
    fn on_codec_config(&mut self, cfg: &CodecConfig);
    fn on_tick(&mut self, now: Instant);
    fn on_rpc_res(&mut self, req_id: u32, res: RpcResult<InternalRpcRes>);
    fn on_transport_rpc_res(&mut self, now: Instant, req_id: EndpointReqId, res: EndpointRes);
    fn on_endpoint_event(&mut self, now: Instant, input: EndpointEvent);
    fn on_str0m_event(&mut self, now: Instant, event: str0m::Event);
    fn close(&mut self, now: Instant);
    fn pop_output(&mut self, now: Instant) -> Option<InternalOutput>;
}

pub struct TransportWebrtc {
    next_tick: Option<Instant>,
    rtc: Rtc,
    rtc_ice_lite: bool,
    internal: Box<dyn TransportWebrtcInternal>,
    ports: Small2dMap<SocketAddr, usize>,
    local_convert: LocalMediaConvert,
    seq_extends: smallmap::Map<Mid, RtpSeqExtend>,
    queue: DynamicDeque<TransportOutput<ExtOut>, 4>,
}

impl TransportWebrtc {
    pub fn new(variant: VariantParams, offer: &str, dtls_cert: DtlsCert, local_addrs: Vec<(SocketAddr, usize)>, rtc_ice_lite: bool) -> RpcResult<(Self, String, String)> {
        let offer = SdpOffer::from_sdp_string(offer).map_err(|_e| RpcError::new2(WebrtcError::InvalidSdp))?;
        let rtc_config = Rtc::builder()
            .set_rtp_mode(true)
            .set_ice_lite(rtc_ice_lite)
            .set_dtls_cert(dtls_cert)
            .set_local_ice_credentials(IceCreds::new())
            .set_stats_interval(Some(Duration::from_secs(1)))
            .set_extension(
                9,
                str0m::rtp::Extension::with_serializer("http://www.webrtc.org/experiments/rtp-hdrext/video-layers-allocation00", str0m::rtp::vla::Serializer),
            )
            .enable_vp8(true)
            .enable_vp9(true)
            .enable_h264(true)
            .enable_opus(true)
            .enable_bwe(Some(Bitrate::kbps(3000)));
        let ice_ufrag = rtc_config.local_ice_credentials().as_ref().expect("should have ice credentials").ufrag.clone();

        let mut rtc = rtc_config.build();
        let mut internal: Box<dyn TransportWebrtcInternal> = match variant {
            VariantParams::Whip(room, peer) => Box::new(whip::TransportWebrtcWhip::new(room, peer)),
            VariantParams::Whep(room, peer) => Box::new(whep::TransportWebrtcWhep::new(room, peer)),
            VariantParams::Webrtc(_ip, _user_agent, req) => {
                rtc.direct_api().create_data_channel(ChannelConfig {
                    label: "data".to_string(),
                    negotiated: Some(1000),
                    ..Default::default()
                });
                //we need to start sctp as client side for handling restart-ice in new server
                //if not, datachannel will not connect successful after reconnect to new server
                rtc.direct_api().start_sctp(true);
                Box::new(webrtc::TransportWebrtcSdk::new(req))
            }
        };

        rtc.direct_api().enable_twcc_feedback();
        let mut ports = Small2dMap::default();
        for (local_addr, slot) in local_addrs {
            ports.insert(local_addr, slot);
            rtc.add_local_candidate(Candidate::host(local_addr, Protocol::Udp).expect("Should add local candidate"));
        }
        let answer = rtc.sdp_api().accept_offer(offer).map_err(|_e| RpcError::new2(WebrtcError::InternalServerError))?;
        let mut local_convert = LocalMediaConvert::default();
        internal.on_codec_config(rtc.codec_config());
        local_convert.set_config(rtc.codec_config());

        Ok((
            Self {
                next_tick: None,
                internal,
                rtc,
                rtc_ice_lite,
                ports,
                local_convert,
                seq_extends: Default::default(),
                queue: Default::default(),
            },
            ice_ufrag,
            answer.to_sdp_string(),
        ))
    }

    fn process_internal_output(&mut self, now: Instant, out: InternalOutput) {
        match out {
            InternalOutput::Str0mKeyframe(mid, kind) => {
                let mut api = self.rtc.direct_api();
                let rx = return_if_none!(api.stream_rx_by_mid(mid, None));
                rx.request_keyframe(kind);
            }
            InternalOutput::Str0mLimitBitrate(mid, bitrate) => {
                log::debug!("[TransportWebrtc] Limit ingress bitrate of track {mid} with {bitrate} bps");
                let mut api = self.rtc.direct_api();
                let rx = return_if_none!(api.stream_rx_by_mid(mid, None));
                rx.request_remb(Bitrate::bps(bitrate));
            }
            InternalOutput::Str0mBwe(current, desired) => {
                log::info!("[TransportWebrtc] Setting str0m bwe {current}, desired {desired}");
                let mut bwe = self.rtc.bwe();
                bwe.set_current_bitrate(current.into());
                bwe.set_desired_bitrate(desired.into());
            }
            InternalOutput::Str0mSendMedia(mid, mut pkt) => {
                let seq_extend = self.seq_extends.entry(mid).or_default();
                let pt = return_if_none!(self.local_convert.convert_codec(pkt.meta.codec()));
                let seq2 = return_if_none!(seq_extend.generate(pkt.seq));
                self.local_convert.rewrite_pkt(&mut pkt);
                log::trace!(
                    "[TransportWebrtc] sending media meta {:?} => pt {pt} seq {} ts {} marker {} payload: {}",
                    pkt.meta,
                    pkt.seq,
                    pkt.ts,
                    pkt.marker,
                    pkt.data.len(),
                );
                let mut api = self.rtc.direct_api();
                let tx = return_if_none!(api.stream_tx_by_mid(mid, None));

                if let Err(e) = tx.write_rtp(pt, seq2.into(), pkt.ts, now, pkt.marker, ExtensionValues::default(), pkt.nackable, pkt.data) {
                    log::error!("[TransportWebrtc] write rtp error {e}");
                }
            }
            InternalOutput::Str0mSendData(channel, data) => {
                let mut channel = return_if_none!(self.rtc.channel(channel));
                if let Err(e) = channel.write(true, &data) {
                    log::error!("[TransportWebrtc] write datachannel error {}", e);
                }
            }
            InternalOutput::Str0mResetBwe(init_bitrate) => {
                log::info!("[TransportWebrtc] Reset str0m bwe to init_bitrate {init_bitrate} bps");
                self.rtc.bwe().reset(init_bitrate.into());
            }
            InternalOutput::TransportOutput(out) => {
                self.queue.push_back(out);
            }
            InternalOutput::RpcReq(req_id, req) => match req {
                InternalRpcReq::SetRemoteSdp(offer) => {
                    if let Ok(offer) = SdpOffer::from_sdp_string(&offer) {
                        if let Ok(answer) = self.rtc.sdp_api().accept_offer(offer) {
                            self.internal.on_rpc_res(req_id, Ok(InternalRpcRes::SetRemoteSdp(answer.to_sdp_string())));
                        } else {
                            self.internal.on_rpc_res(req_id, Err(RpcError::new2(WebrtcError::InternalServerError)));
                        }
                    } else {
                        self.internal.on_rpc_res(req_id, Err(RpcError::new2(WebrtcError::InvalidSdp)));
                    }
                }
            },
        }
    }
}

impl Transport<ExtIn, ExtOut> for TransportWebrtc {
    fn on_tick(&mut self, now: Instant) {
        if let Some(next_tick) = self.next_tick {
            if next_tick <= now {
                self.next_tick = None;
                if let Err(e) = self.rtc.handle_input(str0m::Input::Timeout(now)) {
                    log::error!("[TransportWebrtc] error on timer {}", e);
                }
            }
        }

        self.internal.on_tick(now);
    }

    /// Note: Str0m only stop single incoming packet and we need to pop_output immediate
    /// right after network packet incoming, it not we will lost some media packet.
    /// But the charactis of sans-io-runtime is it will call pop_output after input any event.
    /// Then therefore the network event is not from other task then it will not generate race
    /// between tasks. With this reason we dont need pop from rtc here, and leave it to pop_output function
    fn on_input(&mut self, now: Instant, input: TransportInput<ExtIn>) {
        match input {
            TransportInput::Net(net) => match net {
                BackendIncoming::UdpPacket { slot, from, data } => {
                    let destination = *return_if_none!(self.ports.get2(&slot));
                    log::trace!("[TransportWebrtc] recv udp from {} to {}, len {}", from, destination, data.len());
                    let recv = return_if_err!(Receive::new(Protocol::Udp, from, destination, data.deref()));
                    if let Err(e) = self.rtc.handle_input(str0m::Input::Receive(now, recv)) {
                        log::error!("[TransportWebrtc] handle recv error {}", e);
                    }
                }
                _ => panic!("Unexpected input"),
            },
            TransportInput::Endpoint(event) => {
                self.internal.on_endpoint_event(now, event);
            }
            TransportInput::RpcRes(req_id, res) => {
                self.internal.on_transport_rpc_res(now, req_id, res);
            }
            TransportInput::Ext(ext) => match ext {
                ExtIn::RemoteIce(req_id, variant, ices) => {
                    let mut success_count = 0;
                    for ice in ices {
                        if let Ok(candidate) = Candidate::from_sdp_string(&ice) {
                            success_count += 1;
                            self.rtc.add_remote_candidate(candidate);
                        }
                    }
                    self.queue.push_back(TransportOutput::Ext(ExtOut::RemoteIce(req_id, variant, Ok(success_count))).into());
                }
                ExtIn::RestartIce(req_id, variant, _ip, _useragent, _token, req) => {
                    if let Ok(offer) = SdpOffer::from_sdp_string(&req.sdp) {
                        if let Ok(answer) = self.rtc.sdp_api().accept_offer(offer) {
                            self.internal.on_codec_config(self.rtc.codec_config());
                            self.queue
                                .push_back(TransportOutput::Ext(ExtOut::RestartIce(req_id, variant, Ok((self.rtc_ice_lite, answer.to_sdp_string())))));
                        } else {
                            self.queue
                                .push_back(TransportOutput::Ext(ExtOut::RestartIce(req_id, variant, Err(RpcError::new2(WebrtcError::InternalServerError)))));
                        }
                    } else {
                        self.queue
                            .push_back(TransportOutput::Ext(ExtOut::RestartIce(req_id, variant, Err(RpcError::new2(WebrtcError::InvalidSdp)))));
                    }
                }
            },
            TransportInput::Close => {
                log::info!("[TransportWebrtc] close request");
                self.rtc.disconnect();
                self.internal.close(now);
            }
        }
    }
}

impl TaskSwitcherChild<TransportOutput<ExtOut>> for TransportWebrtc {
    type Time = Instant;

    fn pop_output(&mut self, now: Instant) -> Option<TransportOutput<ExtOut>> {
        return_if_some!(self.queue.pop_front());
        while let Some(out) = self.internal.pop_output(now) {
            self.process_internal_output(now, out);
            return_if_some!(self.queue.pop_front());
        }

        while let Ok(out) = self.rtc.poll_output() {
            match out {
                str0m::Output::Timeout(tick) => {
                    self.next_tick = Some(tick);
                    break;
                }
                str0m::Output::Transmit(out) => {
                    log::trace!("[TransportWebrtc] send udp from {} to {}, len {}", out.source, out.destination, out.contents.len());
                    let from = self.ports.get1(&out.source)?;
                    return Some(TransportOutput::Net(BackendOutgoing::UdpPacket {
                        slot: *from,
                        to: out.destination,
                        data: out.contents.to_vec().into(),
                    }));
                }
                str0m::Output::Event(e) => {
                    self.internal.on_str0m_event(now, e);
                }
            }
        }

        self.queue.pop_front()
    }
}
