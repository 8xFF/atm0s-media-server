use std::{
    net::{IpAddr, SocketAddr},
    ops::Deref,
    time::Instant,
};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointLocalTrackConfig, EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointReq},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, Transport, TransportEvent, TransportInput, TransportOutput, TransportState},
};
use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackPriority, TrackSource},
    media::{MediaKind, MediaMeta, MediaPacket},
};
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    collections::DynamicDeque,
    return_if_none, TaskSwitcherChild,
};
use sdp_rs::SessionDescription;

const REMOTE_AUDIO_TRACK: RemoteTrackId = RemoteTrackId(0);
const LOCAL_AUDIO_TRACK: LocalTrackId = LocalTrackId(0);
const AUDIO_NAME: &str = "audio_main";
const DEFAULT_PRIORITY: TrackPriority = TrackPriority(1);

#[allow(clippy::large_enum_variant)]
pub enum ExtIn {
    Disconnect(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtOut {
    Disconnect(u64, PeerId),
}

pub struct TransportRtpEngine {
    remote: SocketAddr,
    room: RoomId,
    peer: PeerId,
    udp_slot: Option<usize>,
    queue: DynamicDeque<TransportOutput<ExtOut>, 4>,
}

impl TransportRtpEngine {
    pub fn new(room: RoomId, peer: PeerId, ip: IpAddr, offer: &str) -> Result<(Self, String), String> {
        let mut offer = SessionDescription::try_from(offer.to_string()).map_err(|e| e.to_string())?;
        let dest_ip: IpAddr = offer.connection.ok_or("CONNECTION_NOT_FOUND".to_string())?.connection_address.base;
        let dest_port = offer.media_descriptions.pop().ok_or("MEDIA_NOT_FOUND".to_string())?.media.port;
        let remote = SocketAddr::new(dest_ip, dest_port);

        let socket = std::net::UdpSocket::bind(SocketAddr::new(ip, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        let answer = format!(
            "v=0
o=Z 0 1094063179 IN IP4 {ip}
s=Z
c=IN IP4 {ip}
t=0 0
m=audio {port} RTP/AVP 8 101 0
a=rtpmap:101 telephone-event/8000
a=fmtp:101 0-16
a=sendrecv
a=rtcp-mux
"
        );

        Ok((
            Self {
                remote,
                room,
                peer,
                udp_slot: None,
                queue: DynamicDeque::from([
                    TransportOutput::Net(BackendOutgoing::UdpListen {
                        addr: SocketAddr::new(ip, port),
                        reuse: false,
                    }),
                    TransportOutput::Event(TransportEvent::State(TransportState::Connecting(dest_ip))),
                ]),
            },
            answer,
        ))
    }
}

impl Transport<ExtIn, ExtOut> for TransportRtpEngine {
    fn on_tick(&mut self, _now: Instant) {}

    fn on_input(&mut self, _now: Instant, input: TransportInput<ExtIn>) {
        match input {
            TransportInput::Net(event) => self.on_backend(event),
            TransportInput::Endpoint(event) => self.on_event(event),
            TransportInput::RpcRes(_, res) => {
                log::info!("[TransportRtpEngine] on rpc_res {res:?}");
            }
            TransportInput::Ext(ext) => match ext {
                ExtIn::Disconnect(req_id) => {
                    log::info!("[TransportRtpEngine] switched to disconnected with close action from client");
                    self.queue.push_back(TransportOutput::Ext(ExtOut::Disconnect(req_id, self.peer.clone())));
                    self.queue.push_back(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None))));
                }
            },
            TransportInput::SystemClose => {
                self.queue.push_back(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None))));
            }
        }
    }
}

impl TransportRtpEngine {
    fn on_backend(&mut self, event: BackendIncoming) {
        match event {
            BackendIncoming::UdpListenResult { bind, result } => match result {
                Ok((addr, slot)) => {
                    log::info!("[TransportRtpEngine] bind {bind} => {addr} with slot {slot}");
                    log::info!("[TransportRtpEngine] switched to connected");
                    self.udp_slot = Some(slot);
                    self.queue.push_back(TransportOutput::Event(TransportEvent::State(TransportState::Connected(addr.ip()))));
                    self.queue.push_back(TransportOutput::RpcReq(
                        0.into(),
                        EndpointReq::JoinRoom(
                            self.room.clone(),
                            self.peer.clone(),
                            PeerMeta { metadata: None, extra_data: None },
                            RoomInfoPublish { peer: false, tracks: true },
                            RoomInfoSubscribe { peers: false, tracks: true },
                            None,
                        ),
                    ));
                    self.queue.push_back(TransportOutput::Event(TransportEvent::RemoteTrack(
                        REMOTE_AUDIO_TRACK,
                        RemoteTrackEvent::Started {
                            name: AUDIO_NAME.to_string(),
                            priority: TrackPriority(100),
                            meta: TrackMeta::default_audio(),
                        },
                    )));
                    self.queue
                        .push_back(TransportOutput::Event(TransportEvent::LocalTrack(LOCAL_AUDIO_TRACK, LocalTrackEvent::Started(MediaKind::Audio))));
                }
                Err(err) => {
                    log::error!("[TransportRtpEngine] bind {bind} failed {err:?}");
                }
            },
            BackendIncoming::UdpPacket { slot: _, from, data } => {
                log::debug!("[TransportRtpEngine] received from {from} {}", data.len());
                //TODO generate real media_pkt
                let buf = data.deref();
                let pkt_type = pkt_type(buf);
                if let Some(MultiplexKind::Rtp) = pkt_type {
                    if let Ok(rtp) = rtp_rs::RtpReader::new(buf) {
                        log::debug!(
                            "on rtp from {} {} {:?} {} len {}",
                            from,
                            rtp.payload_type(),
                            rtp.sequence_number(),
                            rtp.timestamp(),
                            rtp.payload().len()
                        );
                        let media = MediaPacket {
                            ts: rtp.timestamp(),
                            seq: rtp.sequence_number().into(),
                            marker: rtp.mark(),
                            nackable: false,
                            layers: None,
                            meta: MediaMeta::Opus { audio_level: None },
                            data: rtp.payload().to_vec(),
                        };
                        self.queue
                            .push_back(TransportOutput::Event(TransportEvent::RemoteTrack(REMOTE_AUDIO_TRACK, RemoteTrackEvent::Media(media))));
                    }
                }
            }
        }
    }

    fn on_event(&mut self, event: EndpointEvent) {
        match event {
            EndpointEvent::PeerTrackStarted(peer, track, _) => {
                //TODO select only one or audio_mixer
                if self.peer != peer {
                    log::debug!("[TransportRtpEngine] room {} peer {} attach to {peer}/{track}", self.room, self.peer);
                    self.queue.push_back(TransportOutput::RpcReq(
                        1.into(),
                        EndpointReq::LocalTrack(
                            LOCAL_AUDIO_TRACK,
                            EndpointLocalTrackReq::Attach(
                                TrackSource { peer, track },
                                EndpointLocalTrackConfig {
                                    priority: DEFAULT_PRIORITY,
                                    max_spatial: 2,
                                    max_temporal: 2,
                                    min_spatial: None,
                                    min_temporal: None,
                                },
                            ),
                        ),
                    ));
                }
            }
            EndpointEvent::PeerTrackStopped(peer, track, _) => {
                //TODO select only one or audio_mixer
                if self.peer != peer {
                    log::info!("[TransportRtpEngine] room {} peer {} detach to {peer}/{track}", self.room, self.peer);
                    self.queue
                        .push_back(TransportOutput::RpcReq(1.into(), EndpointReq::LocalTrack(LOCAL_AUDIO_TRACK, EndpointLocalTrackReq::Detach())));
                }
            }
            EndpointEvent::LocalMediaTrack(_track, event) => match event {
                EndpointLocalTrackEvent::Media(media) => {
                    let slot = return_if_none!(self.udp_slot);
                    log::debug!("send rtp to {} {} {} len {}", self.remote, media.seq, media.ts, media.data.len());
                    if let Ok(data) = rtp_rs::RtpPacketBuilder::new()
                        .marked(media.marker)
                        .payload_type(8)
                        .timestamp(media.ts)
                        .sequence(media.seq.into())
                        .payload(&media.data)
                        .build()
                    {
                        self.queue.push_back(TransportOutput::Net(BackendOutgoing::UdpPacket {
                            slot,
                            to: self.remote,
                            data: data.into(),
                        }))
                    }
                }
                EndpointLocalTrackEvent::Status(_) => {}
                EndpointLocalTrackEvent::VoiceActivity(_) => {}
            },
            _ => {}
        }
    }
}

impl TaskSwitcherChild<TransportOutput<ExtOut>> for TransportRtpEngine {
    type Time = Instant;

    fn pop_output(&mut self, _now: Instant) -> Option<TransportOutput<ExtOut>> {
        self.queue.pop_front()
    }
}

pub enum MultiplexKind {
    Rtp,
    Rtcp,
}

fn pkt_type(value: &[u8]) -> Option<MultiplexKind> {
    let byte0 = value[0];
    let len = value.len();

    if byte0 >= 128 && byte0 < 192 && len > 2 {
        let byte1 = value[1];
        let payload_type = byte1 & 0x7F;
        Some(if payload_type < 64 {
            MultiplexKind::Rtp
        } else if payload_type >= 64 && payload_type < 96 {
            MultiplexKind::Rtcp
        } else {
            MultiplexKind::Rtp
        })
    } else {
        None
    }
}
