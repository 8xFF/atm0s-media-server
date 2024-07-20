use std::{
    io,
    marker::PhantomData,
    net::{IpAddr, SocketAddr},
    ops::Deref,
    time::Instant,
};

use media_server_core::{
    endpoint::EndpointEvent,
    transport::{Transport, TransportInput, TransportOutput},
};
use media_server_protocol::{
    endpoint::{PeerId, RoomId},
    transport::RpcResult,
};
use media_server_secure::MediaEdgeSecure;
use rtp::RtpInternal;
use sans_io_runtime::{backend::BackendIncoming, collections::DynamicDeque, return_if_none, return_if_some, Buffer, TaskSwitcherChild};

use crate::sdp::answer_sdp;
mod rtp;

pub enum RtpExtIn {
    Ping(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RtpExtOut {
    // req_id, result
    Pong(u64, RpcResult<String>),
}

#[derive(Debug)]
pub enum VariantParams {
    Rtp(RoomId, PeerId),
}

pub struct InternalNetInput<'a> {
    from: SocketAddr,
    destination: SocketAddr,
    data: &'a [u8],
}

#[derive(Debug, PartialEq, Eq)]
enum InternalOutput {
    SendData(Vec<u8>),
    TransportOutput(TransportOutput<RtpExtOut>),
}

trait TransportRtpInternal {
    fn on_tick(&mut self, now: Instant);
    fn on_endpoint_event(&mut self, now: Instant, event: EndpointEvent);
    fn handle_input(&mut self, input: InternalNetInput) -> Result<(), io::Error>;
    fn pop_output(&mut self, now: Instant) -> Option<InternalOutput>;
}

pub struct TransportRtp<ES> {
    next_tick: Option<Instant>,
    queue: DynamicDeque<TransportOutput<RtpExtOut>, 4>,
    internal: Box<dyn TransportRtpInternal>,
    _tmp: PhantomData<ES>,
}

impl<ES: 'static + MediaEdgeSecure> TransportRtp<ES> {
    pub fn new(params: VariantParams, offer: &str, local_ip: IpAddr, port: u16) -> RpcResult<(Self, SocketAddr, String)> {
        match answer_sdp(offer, local_ip, port) {
            Ok((sdp, remote_ep)) => {
                let internal = match params {
                    VariantParams::Rtp(room, peer) => RtpInternal::new(remote_ep.ip(), room, peer),
                };
                Ok((
                    Self {
                        next_tick: None,
                        queue: Default::default(),
                        internal: Box::new(internal),
                        _tmp: Default::default(),
                    },
                    remote_ep,
                    sdp,
                ))
            }
            Err(err) => Err(err),
        }
    }

    fn process_internal_output(&mut self, now: Instant, out: InternalOutput) {
        match out {
            InternalOutput::SendData(data) => {
                log::trace!("[TransportWebrtc] send data, len {}", data.len());
            }
            InternalOutput::TransportOutput(out) => {
                self.queue.push_back(out);
            }
        }
    }
}

impl<ES: 'static + MediaEdgeSecure> Transport<RtpExtIn, RtpExtOut> for TransportRtp<ES> {
    fn on_tick(&mut self, now: Instant) {
        self.internal.on_tick(now);
    }

    fn on_input(&mut self, now: Instant, input: media_server_core::transport::TransportInput<RtpExtIn>) {
        match input {
            TransportInput::Net(net) => match net {
                BackendIncoming::UdpPacket { slot, from, data } => {
                    log::trace!("[TransportWebrtc] recv udp from {}, len {}", from, data.len());
                    if let Err(err) = self.internal.handle_input(InternalNetInput {
                        from,
                        destination: SocketAddr::from(([0, 0, 0, 0], 8080)),
                        data: data.deref(),
                    }) {
                        log::error!("[TransportWebrtc] error handling input {:?}", err);
                    }
                }
                _ => panic!("unexpected input"),
            },
            TransportInput::Endpoint(ev) => {
                self.internal.on_endpoint_event(now, ev);
            }
            _ => {}
        }
    }
}

impl<ES: 'static + MediaEdgeSecure> TaskSwitcherChild<TransportOutput<RtpExtOut>> for TransportRtp<ES> {
    type Time = Instant;

    fn pop_output(&mut self, now: Self::Time) -> Option<TransportOutput<RtpExtOut>> {
        return_if_some!(self.queue.pop_front());
        while let Some(out) = self.internal.pop_output(now) {
            self.process_internal_output(now, out);
            return_if_some!(self.queue.pop_front());
        }

        self.queue.pop_front()
    }
}
