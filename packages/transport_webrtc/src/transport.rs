use std::{net::SocketAddr, time::Instant};

use media_server_core::transport::{Transport, TransportInput, TransportOutput};
use str0m::{change::SdpOffer, net::Protocol, Candidate, Event as Str0mEvent, Input as Str0mInput, Output as Str0mOutput, Rtc, RtcConfig};

pub enum TransportWebrtcError {
    SdpError,
    RtcError,
}

pub struct TransportWebrtc {
    rtc: Rtc,
    next_tick: Option<Instant>,
}

impl TransportWebrtc {
    pub fn new(offer: &str, local_addrs: Vec<SocketAddr>) -> Result<(Self, String), TransportWebrtcError> {
        let offer = SdpOffer::from_sdp_string(offer).map_err(|_e| TransportWebrtcError::SdpError)?;
        let mut rtc = RtcConfig::new().build();
        for local_addr in local_addrs {
            rtc.add_local_candidate(Candidate::host(local_addr, Protocol::Udp).expect("Should add local candidate"));
        }
        let answer = rtc.sdp_api().accept_offer(offer).map_err(|_e| TransportWebrtcError::RtcError)?;
        Ok((Self { rtc, next_tick: None }, answer.to_sdp_string()))
    }
}

impl Transport for TransportWebrtc {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a>> {
        let next_tick = self.next_tick?;
        if next_tick > now {
            return None;
        }
        self.rtc.handle_input(Str0mInput::Timeout(now)).ok()?;
        self.pop_output(now)
    }

    fn on_input<'a>(&mut self, _now: Instant, _input: TransportInput<'a>) -> Option<TransportOutput<'a>> {
        todo!()
    }

    fn pop_output<'a>(&mut self, _now: Instant) -> Option<TransportOutput<'a>> {
        loop {
            match self.rtc.poll_output().ok()? {
                Str0mOutput::Timeout(instance) => {
                    self.next_tick = Some(instance);
                }
                Str0mOutput::Transmit(_) => {
                    //TODO convert transmit to transport output
                }
                Str0mOutput::Event(event) => match event {
                    Str0mEvent::Connected => todo!(),
                    Str0mEvent::IceConnectionStateChange(_) => todo!(),
                    Str0mEvent::MediaAdded(_) => todo!(),
                    Str0mEvent::MediaData(_) => todo!(),
                    Str0mEvent::MediaChanged(_) => todo!(),
                    Str0mEvent::ChannelOpen(_, _) => todo!(),
                    Str0mEvent::ChannelData(_) => todo!(),
                    Str0mEvent::ChannelClose(_) => todo!(),
                    Str0mEvent::PeerStats(_) => todo!(),
                    Str0mEvent::MediaIngressStats(_) => todo!(),
                    Str0mEvent::MediaEgressStats(_) => todo!(),
                    Str0mEvent::EgressBitrateEstimate(_) => todo!(),
                    Str0mEvent::KeyframeRequest(_) => todo!(),
                    Str0mEvent::StreamPaused(_) => todo!(),
                    Str0mEvent::RtpPacket(_) => todo!(),
                    _ => {}
                },
            }
        }
    }
}
