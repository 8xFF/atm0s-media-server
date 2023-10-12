use std::{
    net::{SocketAddr, UdpSocket},
    os::fd::{AsRawFd, FromRawFd},
    time::Instant,
};

use async_std::prelude::FutureExt;
use str0m::{change::SdpOffer, channel::ChannelId, net::Receive, Candidate, Event, Input, Output, Rtc, RtcError};
use transport::{MediaIncomingEvent, MediaOutgoingEvent, MediaTransport, MediaTransportError, RtpPacket};

use self::{life_cycle::TransportLifeCycle, mid_history::MidHistory};

pub(crate) mod life_cycle;
mod mid_history;
mod utils;

pub enum WebrtcTransportEvent {
    RemoteIce(String),
}

pub struct WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    sync_socket: UdpSocket,
    async_socket: async_std::net::UdpSocket,
    rtc: Rtc,
    life_cycle: L,
    mid_history: MidHistory,
    channel_id: Option<ChannelId>,
    buf: Vec<u8>,
}

impl<L> WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    pub async fn new(life_cycle: L) -> Result<Self, std::io::Error> {
        let addr: SocketAddr = "192.168.66.113:0".parse().expect("Should parse ip address");
        let socket = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, None).expect("Should create socket");
        socket.bind(&addr.into())?;

        let async_socket = unsafe { async_std::net::UdpSocket::from_raw_fd(socket.as_raw_fd()) };
        let sync_socket: UdpSocket = socket.into();

        let rtc = Rtc::builder().set_ice_lite(true).set_rtp_mode(true).build();

        Ok(Self {
            sync_socket,
            async_socket,
            rtc,
            life_cycle,
            mid_history: Default::default(),
            channel_id: None,
            buf: vec![0; 2000],
        })
    }

    pub fn on_remote_sdp(&mut self, sdp: &str) -> Result<String, RtcError> {
        //TODO get ip address
        let addr = self.sync_socket.local_addr().expect("Should has local port");
        let candidate = Candidate::host(addr).expect("Should create candidate");
        self.rtc.add_local_candidate(candidate);

        let sdp = self.rtc.sdp_api().accept_offer(SdpOffer::from_sdp_string(sdp)?)?;
        Ok(sdp.to_sdp_string())
    }
}

#[async_trait::async_trait]
impl<L> MediaTransport<WebrtcTransportEvent> for WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    fn on_event(&mut self, event: MediaOutgoingEvent) -> Result<(), MediaTransportError> {
        Ok(())
    }

    fn on_custom_event(&mut self, event: WebrtcTransportEvent) -> Result<(), MediaTransportError> {
        Ok(())
    }

    async fn recv(&mut self) -> Result<MediaIncomingEvent, MediaTransportError> {
        let timeout = match self.rtc.poll_output() {
            Ok(o) => match o {
                Output::Timeout(t) => t,
                Output::Transmit(t) => {
                    if let Err(_e) = self.async_socket.send_to(&t.contents, t.destination).await {
                        log::error!("Error sending data: {}", _e);
                    }
                    return Ok(MediaIncomingEvent::Continue);
                }
                Output::Event(e) => match e {
                    Event::Connected => {
                        return Ok(self.life_cycle.on_webrtc_connected());
                    }
                    Event::ChannelOpen(chanel_id, name) => {
                        self.channel_id = Some(chanel_id);
                        return Ok(self.life_cycle.on_data_channel(true));
                    }
                    Event::ChannelData(data) => {
                        if !data.binary {
                            if let Ok(data) = String::from_utf8(data.data) {
                                return Ok(MediaIncomingEvent::Data(data));
                            }
                        }
                        return Ok(MediaIncomingEvent::Continue);
                    }
                    Event::ChannelClose(_chanel_id) => {
                        return Ok(self.life_cycle.on_data_channel(false));
                    }
                    Event::IceConnectionStateChange(state) => {
                        return Ok(self.life_cycle.on_ice_state(state));
                    }
                    Event::RtpPacket(rtp) => {
                        let track_id = rtp.header.ext_vals.mid.map(|mid| utils::mid_to_track(&mid));
                        let ssrc: &u32 = &rtp.header.ssrc;
                        if let Some(track_id) = self.mid_history.get(track_id, *ssrc) {
                            log::info!("on rtp {} => {}", rtp.header.ssrc, track_id);
                            return Ok(MediaIncomingEvent::Media(track_id, RtpPacket {}));
                        } else {
                            log::warn!("on rtp without mid {}", rtp.header.ssrc);
                        }
                        return Ok(MediaIncomingEvent::Continue);
                    }
                    _ => {
                        return Ok(MediaIncomingEvent::Continue);
                    }
                },
            },
            Err(e) => {
                todo!("process this error");
            }
        };

        // Duration until timeout.
        let duration = timeout - Instant::now();

        // socket.set_read_timeout(Some(0)) is not ok
        if duration.is_zero() {
            // Drive time forwards in rtc straight away.
            self.rtc.handle_input(Input::Timeout(Instant::now())).unwrap();
            return Ok(MediaIncomingEvent::Continue);
        }

        // Scale up buffer to receive an entire UDP packet.
        self.buf.resize(2000, 0);

        // Try to receive. Because we have a timeout on the socket,
        // we will either receive a packet, or timeout.
        // This is where having an async loop shines. We can await multiple things to
        // happen such as outgoing media data, the timeout and incoming network traffic.
        // When using async there is no need to set timeout on the socket.
        let input = match self.async_socket.recv_from(&mut self.buf).timeout(duration).await {
            Ok(Ok((n, source))) => {
                // UDP data received.
                self.buf.truncate(n);
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
                return Err(MediaTransportError::Other(format!("Error receiving data: {}", e)));
            }
            Err(e) => {
                // Expected error for set_read_timeout().
                // One for windows, one for the rest.
                Input::Timeout(Instant::now())
            }
        };

        // Input is either a Timeout or Receive of data. Both drive the state forward.
        if let Err(e) = self.rtc.handle_input(input) {
            todo!("handle rtc error")
        }
        return Ok(MediaIncomingEvent::Continue);
    }
}
