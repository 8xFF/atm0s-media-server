use std::{
    collections::{HashMap, VecDeque},
    net::{SocketAddr, UdpSocket},
    os::fd::{AsRawFd, FromRawFd},
    time::Instant,
};

use async_std::prelude::FutureExt;
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use str0m::{change::SdpOffer, channel::ChannelId, media::Direction, net::Receive, rtp::ExtensionValues, Candidate, Event, Input, Output, Rtc, RtcError};
use transport::{
    LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, MediaPacketExtensions, MediaSampleRate, RemoteTrackIncomingEvent, RemoteTrackOutgoingEvent, TrackId, TrackMeta, Transport,
    TransportError, TransportIncomingEvent, TransportOutgoingEvent,
};

use crate::rpc::WebrtcConnectRequestSender;

use self::{
    life_cycle::{life_cycle_event_to_event, TransportLifeCycle},
    local_track_id_generator::LocalTrackIdGenerator,
    mid_history::MidHistory,
    msid_alias::MsidAlias,
    rpc::{rpc_from_string, rpc_local_track_to_string, rpc_remote_track_to_string, rpc_to_string, IncomingRpc},
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
    local_track_id_map: HashMap<String, TrackId>,
    remote_track_id_map: HashMap<String, TrackId>,
    local_track_id_gen: LocalTrackIdGenerator,
    channel_id: Option<ChannelId>,
    channel_pending_msgs: VecDeque<String>,
    out_actions: VecDeque<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>>,
    buf: Vec<u8>,
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

        let rtc = Rtc::builder().set_ice_lite(true).set_rtp_mode(true).build();
        log::info!("[TransportWebrtc] created");

        Ok(Self {
            sync_socket,
            async_socket,
            rtc,
            life_cycle,
            msid_alias: Default::default(),
            mid_history: Default::default(),
            local_track_id_map: Default::default(),
            remote_track_id_map: Default::default(),
            local_track_id_gen: Default::default(),
            channel_id: None,
            channel_pending_msgs: Default::default(),
            out_actions: Default::default(),
            buf: vec![0; 2000],
        })
    }

    fn send_msg(&mut self, msg: String) {
        if let Some(channel_id) = self.channel_id {
            if let Some(mut channel) = self.rtc.channel(channel_id) {
                if let Err(e) = channel.write(false, msg.as_bytes()) {
                    log::error!("[WebrtcTransport] error sending data: {}", e);
                }
            }
        } else {
            self.channel_pending_msgs.push_back(msg);
        }
    }

    fn restore_msgs(&mut self) {
        assert!(self.channel_id.is_some());
        while let Some(msg) = self.channel_pending_msgs.pop_front() {
            if let Some(channel_id) = self.channel_id {
                if let Some(mut channel) = self.rtc.channel(channel_id) {
                    if let Err(e) = channel.write(false, msg.as_bytes()) {
                        log::error!("[WebrtcTransport] error sending data: {}", e);
                    }
                }
            }
        }
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
impl<L> Transport<WebrtcTransportEvent, EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn, EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut> for WebrtcTransport<L>
where
    L: TransportLifeCycle,
{
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        if let Some(e) = self.life_cycle.on_tick(now_ms) {
            log::info!("[TransportWebrtc] on new state on tick {:?}", e);
            self.out_actions.push_back(life_cycle_event_to_event(Some(e)));
        }
        Ok(())
    }

    fn on_event(&mut self, now_ms: u64, event: TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>) -> Result<(), TransportError> {
        match event {
            TransportOutgoingEvent::LocalTrackEvent(track_id, event) => match event {
                LocalTrackOutgoingEvent::MediaPacket(pkt) => {
                    let mid = track_to_mid(track_id);
                    if let Some(stream) = self.rtc.direct_api().stream_tx_by_mid(mid, None) {
                        stream
                            .write_rtp(
                                pkt.pt.into(),
                                (pkt.seq_no as u64).into(),
                                pkt.time,
                                Instant::now(),
                                pkt.marker,
                                ExtensionValues { ..Default::default() },
                                true,
                                pkt.payload,
                            )
                            .expect("Should ok");
                    } else {
                        log::warn!("[TransportWebrtc] missing track for mid {}", mid);
                        debug_assert!(false, "should not missing mid");
                    }
                }
                LocalTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_local_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on local track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RemoteTrackEvent(track_id, event) => match event {
                RemoteTrackOutgoingEvent::RequestKeyFrame => {}
                RemoteTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_remote_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on remote track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RequestLimitBitrate(bitrate) => {}
            TransportOutgoingEvent::Rpc(rpc) => {
                let msg = rpc_to_string(rpc);
                log::info!("[TransportWebrtc] on endpoint out rpc: {}", msg);
                self.send_msg(msg);
            }
        }
        Ok(())
    }

    fn on_custom_event(&mut self, now_ms: u64, event: WebrtcTransportEvent) -> Result<(), TransportError> {
        Ok(())
    }

    async fn recv(&mut self, now_ms: u64) -> Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError> {
        if let Some(action) = self.out_actions.pop_front() {
            log::info!("[TransportWebrtc] pop action {:?}", action);
            return action;
        }

        let timeout = match self.rtc.poll_output() {
            Ok(o) => match o {
                Output::Timeout(t) => t,
                Output::Transmit(t) => {
                    if let Err(_e) = self.async_socket.send_to(&t.contents, t.destination).await {
                        log::error!("Error sending data: {}", _e);
                    }
                    return Ok(TransportIncomingEvent::Continue);
                }
                Output::Event(e) => match e {
                    Event::Connected => {
                        return life_cycle_event_to_event(self.life_cycle.on_webrtc_connected(now_ms));
                    }
                    Event::ChannelOpen(chanel_id, name) => {
                        self.channel_id = Some(chanel_id);
                        self.restore_msgs();
                        return life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, true));
                    }
                    Event::ChannelData(data) => {
                        if !data.binary {
                            if let Ok(data) = String::from_utf8(data.data) {
                                match rpc_from_string(&data) {
                                    Ok(IncomingRpc::Endpoint(rpc)) => {
                                        log::info!("[TransportWebrtc] on incoming endpoint rpc: [{:?}]", rpc);
                                        return Ok(TransportIncomingEvent::Rpc(rpc));
                                    }
                                    Ok(IncomingRpc::LocalTrack(track_name, rpc)) => {
                                        if let Some(track_id) = self.local_track_id_map.get(&track_name) {
                                            log::info!("[TransportWebrtc] on incoming local track[{}] rpc: [{:?}]", track_name, rpc);
                                            return Ok(TransportIncomingEvent::LocalTrackEvent(*track_id, LocalTrackIncomingEvent::Rpc(rpc)));
                                        } else {
                                            log::warn!("[TransportWebrtc] on incoming local invalid track[{}] rpc: [{:?}]", track_name, rpc);
                                            return Ok(TransportIncomingEvent::Continue);
                                        }
                                    }
                                    Ok(IncomingRpc::RemoteTrack(track_name, rpc)) => {
                                        if let Some(track_id) = self.remote_track_id_map.get(&track_name) {
                                            log::info!("[TransportWebrtc] on incoming remote track[{}] rpc: [{:?}]", track_name, rpc);
                                            return Ok(TransportIncomingEvent::RemoteTrackEvent(*track_id, RemoteTrackIncomingEvent::Rpc(rpc)));
                                        } else {
                                            log::warn!("[TransportWebrtc] on incoming remote invalid track[{}] rpc: [{:?}]", track_name, rpc);
                                            return Ok(TransportIncomingEvent::Continue);
                                        }
                                    }
                                    _ => {
                                        log::warn!("[TransportWebrtc] invalid rpc: {}", data);
                                        return Ok(TransportIncomingEvent::Continue);
                                    }
                                }
                            }
                        }
                        return Ok(TransportIncomingEvent::Continue);
                    }
                    Event::ChannelClose(_chanel_id) => {
                        return life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, false));
                    }
                    Event::IceConnectionStateChange(state) => {
                        return life_cycle_event_to_event(self.life_cycle.on_ice_state(now_ms, state));
                    }
                    Event::RtpPacket(rtp) => {
                        let track_id = rtp.header.ext_vals.mid.map(|mid| utils::mid_to_track(&mid));
                        let ssrc: &u32 = &rtp.header.ssrc;
                        if let Some(track_id) = self.mid_history.get(track_id, *ssrc) {
                            // log::info!("on rtp {} => {}", rtp.header.ssrc, track_id);
                            return Ok(TransportIncomingEvent::RemoteTrackEvent(
                                track_id,
                                RemoteTrackIncomingEvent::MediaPacket(MediaPacket {
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
                                }),
                            ));
                        } else {
                            log::warn!("on rtp without mid {}", rtp.header.ssrc);
                        }
                        return Ok(TransportIncomingEvent::Continue);
                    }
                    Event::MediaAdded(added) => {
                        if let Some(media) = self.rtc.media(added.mid) {
                            match added.direction {
                                Direction::RecvOnly => {
                                    //remote stream
                                    let track_id = utils::mid_to_track(&added.mid);
                                    let msid = media.msid();
                                    if let Some(info) = self.msid_alias.get_alias(&msid.stream_id, &msid.track_id) {
                                        self.remote_track_id_map.insert(info.name.clone(), track_id);
                                        log::info!("[TransportWebrtc] added remote track {} => {} added {:?} {:?}", info.name, track_id, added, info);
                                        return Ok(TransportIncomingEvent::RemoteTrackAdded(
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
                                    log::info!("[TransportWebrtc] added local track {} => {} added {:?}", track_name, track_id, added);
                                    self.local_track_id_map.insert(track_name.clone(), track_id);
                                    return Ok(TransportIncomingEvent::LocalTrackAdded(
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
                        return Ok(TransportIncomingEvent::Continue);
                    }
                    Event::MediaChanged(media) => {
                        //TODO
                        return Ok(TransportIncomingEvent::Continue);
                    }
                    Event::StreamPaused(paused) => {
                        //TODO
                        return Ok(TransportIncomingEvent::Continue);
                    }
                    _ => {
                        return Ok(TransportIncomingEvent::Continue);
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
            return Ok(TransportIncomingEvent::Continue);
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
                log::error!("[TransportWebrtc] network eror {:?}", e);
                return Err(TransportError::NetworkError);
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
        return Ok(TransportIncomingEvent::Continue);
    }
}

impl<L: TransportLifeCycle> Drop for WebrtcTransport<L> {
    fn drop(&mut self) {
        log::info!("[TransportWebrtc] drop");
    }
}
