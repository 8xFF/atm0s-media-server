use std::{
    marker::PhantomData,
    net::{IpAddr, SocketAddr},
    time::Instant,
};

use media_server_core::transport::{Transport, TransportOutput};
use media_server_protocol::{
    endpoint::{PeerId, RoomId},
    transport::RpcResult,
};
use media_server_secure::MediaEdgeSecure;
use sans_io_runtime::{collections::DynamicDeque, TaskSwitcherChild};

use crate::sdp::answer_sdp;

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

pub struct TransportRtp<ES> {
    next_tick: Option<Instant>,
    queue: DynamicDeque<TransportOutput<RtpExtOut>, 4>,
    _tmp: PhantomData<ES>,
}

impl<ES: 'static + MediaEdgeSecure> TransportRtp<ES> {
    pub fn new(params: VariantParams, offer: &str, local_ip: IpAddr, port: u16) -> RpcResult<(Self, SocketAddr, String)> {
        match answer_sdp(offer, local_ip, port) {
            Ok((sdp, remote_ep)) => Ok((
                Self {
                    next_tick: None,
                    queue: Default::default(),
                    _tmp: Default::default(),
                },
                remote_ep,
                sdp,
            )),
            Err(err) => Err(err),
        }
    }
}

impl<ES: 'static + MediaEdgeSecure> Transport<RtpExtIn, RtpExtOut> for TransportRtp<ES> {
    fn on_tick(&mut self, now: Instant) {}

    fn on_input(&mut self, now: Instant, input: media_server_core::transport::TransportInput<RtpExtIn>) {}
}

impl<ES: 'static + MediaEdgeSecure> TaskSwitcherChild<TransportOutput<RtpExtOut>> for TransportRtp<ES> {
    type Time = Instant;

    fn pop_output(&mut self, now: Self::Time) -> Option<TransportOutput<RtpExtOut>> {
        self.queue.pop_front()
    }
}
