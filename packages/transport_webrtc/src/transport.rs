use std::{
    net::SocketAddr,
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
    transport::{RpcError, RpcResult},
};
use media_server_utils::Small2dMap;
use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};
use str0m::{
    bwe::Bitrate,
    change::{DtlsCert, SdpOffer},
    ice::IceCreds,
    media::{KeyframeRequestKind, Mid},
    net::{Protocol, Receive},
    rtp::ExtensionValues,
    Candidate, Rtc,
};

use crate::WebrtcError;

mod bwe_state;
mod whep;
mod whip;

pub enum VariantParams {
    Whip(RoomId, PeerId),
    Whep(RoomId, PeerId),
    Sdk,
}

pub enum Variant {
    Whip,
    Whep,
    Sdk,
}

pub enum ExtIn {
    RemoteIce(u64, Variant, String),
}

pub enum ExtOut {
    RemoteIce(u64, Variant, RpcResult<()>),
}

enum InternalOutput<'a> {
    Str0mKeyframe(Mid, KeyframeRequestKind),
    Str0mLimitBitrate(Mid, u64),
    Str0mSendMedia(Mid, MediaPacket),
    Str0mBwe(u64, u64),
    Str0mResetBwe(u64),
    TransportOutput(TransportOutput<'a, ExtOut>),
}

trait TransportWebrtcInternal {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>>;
    fn on_transport_rpc_res<'a>(&mut self, now: Instant, req_id: EndpointReqId, res: EndpointRes) -> Option<InternalOutput<'a>>;
    fn on_endpoint_event<'a>(&mut self, now: Instant, input: EndpointEvent) -> Option<InternalOutput<'a>>;
    fn on_str0m_event<'a>(&mut self, now: Instant, event: str0m::Event) -> Option<InternalOutput<'a>>;
    fn close<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>>;
    fn pop_output<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>>;
}

pub struct TransportWebrtc {
    next_tick: Option<Instant>,
    rtc: Rtc,
    internal: Box<dyn TransportWebrtcInternal>,
    ports: Small2dMap<SocketAddr, usize>,
}

impl TransportWebrtc {
    pub fn new(variant: VariantParams, offer: &str, dtls_cert: DtlsCert, local_addrs: Vec<(SocketAddr, usize)>) -> RpcResult<(Self, String, String)> {
        let offer = SdpOffer::from_sdp_string(offer).map_err(|_e| RpcError::new2(WebrtcError::SdpError))?;
        let rtc_config = Rtc::builder()
            .set_rtp_mode(true)
            .set_ice_lite(true)
            .set_dtls_cert(dtls_cert)
            .set_local_ice_credentials(IceCreds::new())
            .set_stats_interval(Some(Duration::from_secs(1)))
            .enable_bwe(Some(Bitrate::kbps(3000)));
        let ice_ufrag = rtc_config.local_ice_credentials().as_ref().expect("should have ice credentials").ufrag.clone();

        let mut rtc = rtc_config.build();
        rtc.direct_api().enable_twcc_feedback();

        let mut ports = Small2dMap::default();
        for (local_addr, slot) in local_addrs {
            ports.insert(local_addr, slot);
            rtc.add_local_candidate(Candidate::host(local_addr, Protocol::Udp).expect("Should add local candidate"));
        }
        let answer = rtc.sdp_api().accept_offer(offer).map_err(|_e| RpcError::new2(WebrtcError::Str0mError))?;

        Ok((
            Self {
                next_tick: None,
                rtc,
                internal: match variant {
                    VariantParams::Whip(room, peer) => Box::new(whip::TransportWebrtcWhip::new(room, peer)),
                    VariantParams::Whep(room, peer) => Box::new(whep::TransportWebrtcWhep::new(room, peer)),
                    VariantParams::Sdk => unimplemented!(),
                },
                ports,
            },
            ice_ufrag,
            answer.to_sdp_string(),
        ))
    }

    fn process_internal_output<'a>(&mut self, now: Instant, out: InternalOutput<'a>) -> Option<TransportOutput<'a, ExtOut>> {
        match out {
            InternalOutput::Str0mKeyframe(mid, kind) => {
                self.rtc.direct_api().stream_rx_by_mid(mid, None)?.request_keyframe(kind);
                self.pop_event(now)
            }
            InternalOutput::Str0mLimitBitrate(mid, bitrate) => {
                log::debug!("Limit remote tracks with Remb {bitrate}");
                self.rtc.direct_api().stream_rx_by_mid(mid, None)?.request_remb(Bitrate::bps(bitrate));
                self.pop_event(now)
            }
            InternalOutput::Str0mBwe(current, desired) => {
                log::debug!("Setting str0m bwe {current}, desired {desired}");
                let mut bwe = self.rtc.bwe();
                bwe.set_current_bitrate(current.into());
                bwe.set_desired_bitrate(desired.into());
                self.pop_event(now)
            }
            InternalOutput::Str0mSendMedia(mid, pkt) => {
                log::trace!("[TransportWebrtc] sending media payload {} seq {} to mid {mid}", pkt.pt, pkt.seq);
                self.rtc
                    .direct_api()
                    .stream_tx_by_mid(mid, None)?
                    .write_rtp(pkt.pt.into(), pkt.seq.into(), pkt.ts, now, pkt.marker, ExtensionValues::default(), pkt.nackable, pkt.data)
                    .ok()?;
                self.pop_event(now)
            }
            InternalOutput::Str0mResetBwe(init_bitrate) => {
                log::info!("Reset str0m bwe to init_bitrate {init_bitrate} bps");
                self.rtc.bwe().reset(init_bitrate.into());
                self.pop_event(now)
            }
            InternalOutput::TransportOutput(out) => Some(out),
        }
    }
}

impl Transport<ExtIn, ExtOut> for TransportWebrtc {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a, ExtOut>> {
        if let Some(next_tick) = self.next_tick {
            if next_tick <= now {
                self.next_tick = None;
                self.rtc.handle_input(str0m::Input::Timeout(now)).ok()?;
                return self.pop_event(now);
            }
        }

        let out = self.internal.on_tick(now)?;
        self.process_internal_output(now, out)
    }

    fn on_input<'a>(&mut self, now: Instant, input: TransportInput<'a, ExtIn>) -> Option<TransportOutput<'a, ExtOut>> {
        match input {
            TransportInput::Net(net) => match net {
                BackendIncoming::UdpPacket { slot, from, data } => {
                    let destination = *self.ports.get2(&slot)?;
                    log::trace!("[TransportWebrtc] recv udp from {} to {}, len {}", from, destination, data.len());
                    self.rtc
                        .handle_input(str0m::Input::Receive(now, Receive::new(Protocol::Udp, from, destination, data.deref()).ok()?))
                        .ok()?;
                    self.pop_event(now)
                }
                _ => panic!("Unexpected input"),
            },
            TransportInput::Endpoint(event) => {
                let out = self.internal.on_endpoint_event(now, event)?;
                self.process_internal_output(now, out)
            }
            TransportInput::RpcRes(req_id, res) => {
                let out = self.internal.on_transport_rpc_res(now, req_id, res)?;
                self.process_internal_output(now, out)
            }
            TransportInput::Ext(ext) => match ext {
                ExtIn::RemoteIce(req_id, variant, _ice) => {
                    //TODO handle remote-ice with str0m
                    Some(TransportOutput::Ext(ExtOut::RemoteIce(req_id, variant, Ok(()))))
                }
            },
            TransportInput::Close => {
                log::info!("[TransportWebrtc] close request");
                self.rtc.disconnect();
                if let Some(out) = self.internal.close(now) {
                    self.process_internal_output(now, out)
                } else {
                    self.pop_event(now)
                }
            }
        }
    }

    fn pop_event<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a, ExtOut>> {
        while let Some(out) = self.internal.pop_output(now) {
            let out = self.process_internal_output(now, out);
            if out.is_some() {
                return out;
            }
        }

        loop {
            let out = self.rtc.poll_output().ok()?;
            match out {
                str0m::Output::Timeout(tick) => {
                    self.next_tick = Some(tick);
                    return None;
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
                    if let Some(out) = self.internal.on_str0m_event(now, e) {
                        let out = self.process_internal_output(now, out);
                        if out.is_some() {
                            return out;
                        }
                    }
                }
            }
        }
    }
}
