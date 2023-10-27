use std::{
    net::{SocketAddr, UdpSocket},
    os::fd::{AsRawFd, FromRawFd},
    time::{Duration, Instant},
};

use async_std::prelude::FutureExt;
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut, RpcResponse},
    EndpointRpcIn, EndpointRpcOut,
};
use str0m::{
    change::SdpOffer,
    channel::ChannelId,
    media::{KeyframeRequestKind, MediaTime, Mid},
    net::Receive,
    rtp::ExtensionValues,
    Candidate, Input, Output, Rtc, RtcError,
};
use transport::{MediaPacket, Transport, TransportError, TransportIncomingEvent, TransportOutgoingEvent};

use crate::rpc::WebrtcConnectRequestSender;

use self::{
    internal::{
        life_cycle::TransportLifeCycle,
        rpc::{TransportRpcIn, TransportRpcOut, UpdateSdpResponse},
        rtp_packet_convert::MediaPacketConvert,
        WebrtcTransportInternal,
    },
    sdp_box::SdpBox,
};

pub(crate) mod internal;
pub mod sdp_box;

const INIT_BWE_BITRATE_KBPS: u64 = 400; //400kbps

#[derive(Debug, PartialEq, Eq)]
pub enum Str0mAction {
    Media(Mid, MediaPacket),
    RequestKeyFrame(Mid),
    Datachannel(ChannelId, String),
    Rpc(TransportRpcIn),
}

pub enum WebrtcTransportEvent {
    RemoteIce(String),
}

pub struct WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    sync_socket: UdpSocket,
    async_socket: async_std::net::UdpSocket,
    sdp_box: SdpBox,
    rtc: Rtc,
    internal: WebrtcTransportInternal<L>,
    buf: Vec<u8>,
    pkt_convert: MediaPacketConvert,
}

impl<L> WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    pub async fn new(life_cycle: L) -> Result<Self, std::io::Error> {
        let addr: SocketAddr = "127.0.0.1:0".parse().expect("Should parse ip address");
        let socket = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, None).expect("Should create socket");
        socket.bind(&addr.into())?;

        let async_socket = unsafe { async_std::net::UdpSocket::from_raw_fd(socket.as_raw_fd()) };
        let sync_socket: UdpSocket = socket.into();

        let rtc = Rtc::builder()
            // .enable_bwe(Some(Bitrate::kbps(INIT_BWE_BITRATE_KBPS)))
            .set_ice_lite(true)
            .set_rtp_mode(true)
            .set_stats_interval(Some(Duration::from_millis(500)))
            .build();
        log::info!("[TransportWebrtc] created");

        Ok(Self {
            sync_socket,
            async_socket,
            sdp_box: Default::default(),
            rtc,
            internal: WebrtcTransportInternal::new(life_cycle),
            buf: vec![0; 2000],
            pkt_convert: Default::default(),
        })
    }

    pub fn map_remote_stream(&mut self, sender: WebrtcConnectRequestSender) {
        self.internal.map_remote_stream(sender);
    }

    pub fn on_remote_sdp(&mut self, sdp: &str) -> Result<String, RtcError> {
        //TODO get ip address
        let addr = self.sync_socket.local_addr().expect("Should has local port");
        let candidate = Candidate::host(addr).expect("Should create candidate");
        self.rtc.add_local_candidate(candidate);

        let mut sdp_offer = SdpOffer::from_sdp_string(sdp)?;
        let sdp_answer = self.rtc.sdp_api().accept_offer(sdp_offer)?;
        Ok(self.sdp_box.rewrite_answer(&sdp_answer.to_sdp_string()))
    }

    fn pop_internal_str0m_actions(&mut self, now_ms: u64) {
        while let Some(action) = self.internal.str0m_action() {
            match action {
                Str0mAction::Rpc(rpc) => match rpc {
                    TransportRpcIn::UpdateSdp(req) => {
                        if let Ok(mut sdp_offer) = SdpOffer::from_sdp_string(&req.data.sdp) {
                            for sender in req.data.senders {
                                self.internal.map_remote_stream(sender);
                            }
                            match self.rtc.sdp_api().accept_offer(sdp_offer) {
                                Ok(sdp_answer) => {
                                    let sdp = self.sdp_box.rewrite_answer(&sdp_answer.to_sdp_string());
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
                Str0mAction::Media(mid, mut pkt) => {
                    if let Some(stream) = self.rtc.direct_api().stream_tx_by_mid(mid, None) {
                        self.pkt_convert.rewrite_codec(&mut pkt);
                        stream
                            .write_rtp(
                                self.pkt_convert.to_pt(&pkt),
                                (pkt.seq_no as u64).into(),
                                pkt.time,
                                Instant::now(),
                                pkt.marker,
                                ExtensionValues {
                                    abs_send_time: pkt.ext_vals.abs_send_time.map(|t| MediaTime::new(t.0, t.1)),
                                    transport_cc: pkt.ext_vals.transport_cc,
                                    ..Default::default()
                                },
                                pkt.nackable,
                                pkt.payload,
                            )
                            .expect("Should ok");
                    } else {
                        log::warn!("[TransportWebrtc] missing track for mid {}", mid);
                        debug_assert!(false, "should not missing mid");
                    }
                }
                Str0mAction::RequestKeyFrame(mid) => {
                    if let Some(stream) = self.rtc.direct_api().stream_rx_by_mid(mid, None) {
                        stream.request_keyframe(KeyframeRequestKind::Pli);
                    } else {
                        log::warn!("[TransportWebrtc] missing track for mid {} when requesting key-frame", mid);
                        debug_assert!(false, "should not missing mid");
                    }
                }
                Str0mAction::Datachannel(cid, msg) => {
                    if let Some(mut channel) = self.rtc.channel(cid) {
                        if let Err(e) = channel.write(false, msg.as_bytes()) {
                            log::error!("[TransportWebrtc] write datachannel error {:?}", e);
                        }
                    } else {
                        log::warn!("[TransportWebrtc] missing channel for id {:?}", cid);
                        debug_assert!(false, "should not missing channel id");
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
                    if let Err(_e) = self.async_socket.send_to(&t.contents, t.destination).await {
                        log::error!("Error sending data: {}", _e);
                    }
                    return Ok(TransportIncomingEvent::Continue);
                }
                Output::Event(e) => {
                    self.internal.on_str0m_event(now_ms, e)?;
                    if let Some(action) = self.internal.endpoint_action() {
                        return action;
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
            self.rtc.handle_input(Input::Timeout(Instant::now())).unwrap();
            return Ok(TransportIncomingEvent::Continue);
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
        let input = match self.async_socket.recv_from(&mut self.buf).timeout(duration).await {
            Ok(Ok((n, source))) => {
                // UDP data received.
                unsafe {
                    self.buf.set_len(n);
                }
                Input::Receive(
                    Instant::now(),
                    Receive {
                        source,
                        destination: self.async_socket.local_addr().expect("Should has local_addr"),
                        contents: self.buf.as_slice().try_into().unwrap(),
                    },
                )
            }
            Ok(Err(e)) => {
                log::error!("[TransportWebrtc] network eror {:?}", e);
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

    async fn close(&mut self) {
        //TODO force close this
    }
}

impl<L: TransportLifeCycle> Drop for WebrtcTransport<L> {
    fn drop(&mut self) {
        log::info!("[TransportWebrtc] drop");
    }
}
