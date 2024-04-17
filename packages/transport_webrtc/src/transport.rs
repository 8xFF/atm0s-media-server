use std::{net::SocketAddr, ops::Deref, time::Instant};

use media_server_core::transport::{Transport, TransportInput, TransportOutput};
use media_server_protocol::media::MediaPacket;
use sans_io_runtime::Buffer;
use str0m::{
    change::{DtlsCert, SdpOffer},
    ice::IceCreds,
    media::{KeyframeRequestKind, Mid},
    net::{Protocol, Receive},
    rtp::ExtensionValues,
    Candidate, Rtc,
};

mod whep;
mod whip;

pub enum Variant {
    Whip,
    Whep,
    Sdk,
}

enum InternalOutput<'a> {
    Str0mReceive(Instant, Protocol, SocketAddr, SocketAddr, Buffer<'a>),
    Str0mTick(Instant),
    Str0mKeyframe(Mid, KeyframeRequestKind),
    Str0mSendMedia(Mid, MediaPacket),
    TransportOutput(TransportOutput<'a>),
}

trait TransportWebrtcInternal {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<InternalOutput<'a>>;
    fn on_transport_input<'a>(&mut self, now: Instant, input: TransportInput<'a>) -> Option<InternalOutput<'a>>;
    fn on_str0m_out<'a>(&mut self, now: Instant, out: str0m::Output) -> Option<InternalOutput<'a>>;
}

pub enum TransportWebrtcError {
    SdpError,
    RtcError,
}

pub struct TransportWebrtc {
    rtc: Rtc,
    internal: Box<dyn TransportWebrtcInternal>,
}

impl TransportWebrtc {
    pub fn new(variant: Variant, offer: &str, dtls_cert: DtlsCert, local_addrs: Vec<(SocketAddr, usize)>) -> Result<(Self, String, String), TransportWebrtcError> {
        let offer = SdpOffer::from_sdp_string(offer).map_err(|_e| TransportWebrtcError::SdpError)?;
        let rtc_config = Rtc::builder().set_rtp_mode(true).set_ice_lite(true).set_dtls_cert(dtls_cert).set_local_ice_credentials(IceCreds::new());
        let ice_ufrag = rtc_config.local_ice_credentials().as_ref().expect("should have ice credentials").ufrag.clone();

        let mut rtc = rtc_config.build();
        rtc.direct_api().enable_twcc_feedback();

        for (local_addr, _slot) in &local_addrs {
            rtc.add_local_candidate(Candidate::host(*local_addr, Protocol::Udp).expect("Should add local candidate"));
        }
        let answer = rtc.sdp_api().accept_offer(offer).map_err(|_e| TransportWebrtcError::RtcError)?;
        Ok((
            Self {
                rtc,
                internal: match variant {
                    Variant::Whip => Box::new(whip::TransportWebrtcWhip::new(local_addrs)),
                    Variant::Whep => Box::new(whep::TransportWebrtcWhep::new(local_addrs)),
                    Variant::Sdk => unimplemented!(),
                },
            },
            ice_ufrag,
            answer.to_sdp_string(),
        ))
    }

    fn process_internal_output<'a>(&mut self, now: Instant, out: InternalOutput<'a>) -> Option<TransportOutput<'a>> {
        match out {
            InternalOutput::Str0mReceive(now, protocol, source, destination, buf) => {
                self.rtc.handle_input(str0m::Input::Receive(now, Receive::new(protocol, source, destination, buf.deref()).ok()?)).ok()?;
                self.pop_output(now)
            }
            InternalOutput::Str0mTick(now) => {
                self.rtc.handle_input(str0m::Input::Timeout(now)).ok()?;
                self.pop_output(now)
            }
            InternalOutput::Str0mKeyframe(mid, kind) => {
                self.rtc.direct_api().stream_rx_by_mid(mid, None)?.request_keyframe(kind);
                self.pop_output(now)
            }
            InternalOutput::Str0mSendMedia(mid, pkt) => {
                self.rtc
                    .direct_api()
                    .stream_tx_by_mid(mid, None)?
                    .write_rtp(pkt.pt.into(), pkt.seq.into(), pkt.ts, now, pkt.marker, ExtensionValues::default(), pkt.nackable, pkt.data)
                    .ok()?;
                self.pop_output(now)
            }
            InternalOutput::TransportOutput(out) => Some(out),
        }
    }
}

impl Transport for TransportWebrtc {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a>> {
        let out = self.internal.on_tick(now)?;
        self.process_internal_output(now, out)
    }

    fn on_input<'a>(&mut self, now: Instant, input: TransportInput<'a>) -> Option<TransportOutput<'a>> {
        let out = self.internal.on_transport_input(now, input)?;
        self.process_internal_output(now, out)
    }

    fn pop_output<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a>> {
        loop {
            let out = self.rtc.poll_output().ok()?;
            if let Some(out) = self.internal.on_str0m_out(now, out) {
                let out = self.process_internal_output(now, out);
                if out.is_some() {
                    return out;
                }
            }
        }
    }
}
