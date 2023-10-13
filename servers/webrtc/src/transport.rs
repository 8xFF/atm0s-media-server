use std::{
    net::{SocketAddr, UdpSocket},
    os::fd::{AsRawFd, FromRawFd},
    time::Instant,
};

use async_std::prelude::FutureExt;
use endpoint::{EndpointRpcIn, EndpointRpcOut};
use str0m::{change::SdpOffer, channel::ChannelId, media::Direction, net::Receive, rtp::ExtensionValues, Candidate, Event, Input, Output, Rtc, RtcError};
use transport::{MediaIncomingEvent, MediaOutgoingEvent, MediaPacket, MediaPacketExtensions, MediaSampleRate, MediaTransport, MediaTransportError, TrackMeta};

use crate::rpc::WebrtcConnectRequestSender;

use self::{
    life_cycle::{life_cycle_event_to_event, TransportLifeCycle},
    local_track_id_generator::LocalTrackIdGenerator,
    mid_history::MidHistory,
    msid_alias::MsidAlias,
    rpc::{rpc_from_string, rpc_to_string},
    utils::{to_transport_kind, track_to_mid},
};

pub(crate) mod life_cycle;
mod local_track_id_generator;
mod mid_history;
mod msid_alias;
mod rpc;
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
    msid_alias: MsidAlias,
    mid_history: MidHistory,
    local_track_id_gen: LocalTrackIdGenerator,
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
            msid_alias: Default::default(),
            mid_history: Default::default(),
            local_track_id_gen: Default::default(),
            channel_id: None,
            buf: vec![0; 2000],
        })
    }

    pub fn map_remote_stream(&mut self, sender: WebrtcConnectRequestSender) {
        self.msid_alias.add_alias(&sender.uuid, &sender.label, &sender.kind, &sender.name);
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
impl<L> MediaTransport<WebrtcTransportEvent, EndpointRpcIn, EndpointRpcOut> for WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    fn on_event(&mut self, event: MediaOutgoingEvent<EndpointRpcOut>) -> Result<(), MediaTransportError> {
        match event {
            MediaOutgoingEvent::Media(track_id, pkt) => {
                let mid = track_to_mid(track_id);
                if let Some(stream) = self.rtc.direct_api().stream_tx_by_mid(mid, None) {
                    stream.write_rtp(
                        pkt.pt.into(),
                        (pkt.seq_no as u64).into(),
                        pkt.time,
                        Instant::now(),
                        pkt.marker,
                        ExtensionValues { ..Default::default() },
                        true,
                        pkt.payload,
                    );
                }
            }
            MediaOutgoingEvent::RequestPli(track_id) => {}
            MediaOutgoingEvent::RequestSli(track_id) => {}
            MediaOutgoingEvent::RequestLimitBitrate(bitrate) => {}
            MediaOutgoingEvent::Rpc(rpc) => {
                let msg = rpc_to_string(rpc);
                if let Some(channel_id) = self.channel_id {
                    if let Some(mut channel) = self.rtc.channel(channel_id) {
                        if let Err(e) = channel.write(false, msg.as_bytes()) {
                            log::error!("[WebrtcTransport] error sending data: {}", e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn on_custom_event(&mut self, event: WebrtcTransportEvent) -> Result<(), MediaTransportError> {
        Ok(())
    }

    async fn recv(&mut self) -> Result<MediaIncomingEvent<EndpointRpcIn>, MediaTransportError> {
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
                        return life_cycle_event_to_event(self.life_cycle.on_webrtc_connected());
                    }
                    Event::ChannelOpen(chanel_id, name) => {
                        self.channel_id = Some(chanel_id);
                        return life_cycle_event_to_event(self.life_cycle.on_data_channel(true));
                    }
                    Event::ChannelData(data) => {
                        if !data.binary {
                            if let Ok(data) = String::from_utf8(data.data) {
                                if let Ok(rpc) = rpc_from_string(&data) {
                                    return Ok(MediaIncomingEvent::Rpc(rpc));
                                }
                            }
                        }
                        return Ok(MediaIncomingEvent::Continue);
                    }
                    Event::ChannelClose(_chanel_id) => {
                        return life_cycle_event_to_event(self.life_cycle.on_data_channel(false));
                    }
                    Event::IceConnectionStateChange(state) => {
                        return life_cycle_event_to_event(self.life_cycle.on_ice_state(state));
                    }
                    Event::RtpPacket(rtp) => {
                        let track_id = rtp.header.ext_vals.mid.map(|mid| utils::mid_to_track(&mid));
                        let ssrc: &u32 = &rtp.header.ssrc;
                        if let Some(track_id) = self.mid_history.get(track_id, *ssrc) {
                            // log::info!("on rtp {} => {}", rtp.header.ssrc, track_id);
                            return Ok(MediaIncomingEvent::RemoteTrackMedia(
                                track_id,
                                MediaPacket {
                                    pt: *(&rtp.header.payload_type as &u8),
                                    seq_no: rtp.header.sequence_number,
                                    time: rtp.header.timestamp,
                                    marker: rtp.header.marker,
                                    ext_vals: MediaPacketExtensions {
                                        abs_send_time: rtp.header.ext_vals.abs_send_time.map(|t| (t.numer(), t.denom())),
                                        transport_cc: rtp.header.ext_vals.transport_cc,
                                    },
                                    nackable: true,
                                    payload: rtp.payload,
                                },
                            ));
                        } else {
                            log::warn!("on rtp without mid {}", rtp.header.ssrc);
                        }
                        return Ok(MediaIncomingEvent::Continue);
                    }
                    Event::MediaAdded(added) => {
                        if let Some(media) = self.rtc.media(added.mid) {
                            match added.direction {
                                Direction::RecvOnly => {
                                    //remote stream
                                    let track_id = utils::mid_to_track(&added.mid);
                                    let msid = media.msid();
                                    if let Some(info) = self.msid_alias.get_alias(&msid.stream_id, &msid.track_id) {
                                        log::info!("media added {:?} {:?}", added, info);
                                        return Ok(MediaIncomingEvent::RemoteTrackAdded(
                                            info.name,
                                            track_id,
                                            TrackMeta {
                                                kind: to_transport_kind(added.kind),
                                                sample_rate: MediaSampleRate::HzCustom(0), //TODO
                                                label: Some(info.label),
                                            },
                                        ));
                                    }
                                }
                                Direction::SendOnly => {
                                    //local stream
                                    let track_id = utils::mid_to_track(&added.mid);
                                    let track_name = self.local_track_id_gen.generate(added.kind, added.mid);
                                    return Ok(MediaIncomingEvent::LocalTrackAdded(
                                        track_name,
                                        track_id,
                                        TrackMeta {
                                            kind: to_transport_kind(added.kind),
                                            sample_rate: MediaSampleRate::HzCustom(0), //TODO
                                            label: None,
                                        },
                                    ));
                                }
                                _ => {
                                    panic!("not supported")
                                }
                            }
                        }
                        return Ok(MediaIncomingEvent::Continue);
                    }
                    Event::MediaChanged(media) => {
                        //TODO
                        return Ok(MediaIncomingEvent::Continue);
                    }
                    Event::StreamPaused(paused) => {
                        //TODO
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
