use std::{
    net::{IpAddr, SocketAddr},
    ops::Deref,
    time::{Duration, Instant},
};

use media_server_codecs::{
    opus::{OpusDecoder, OpusEncoder},
    pcma::{PcmaDecoder, PcmaEncoder},
    AudioTranscoder,
};
use media_server_core::{
    endpoint::{EndpointEvent, EndpointLocalTrackConfig, EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointReq},
    transport::{LocalTrackEvent, LocalTrackId, RemoteTrackEvent, RemoteTrackId, Transport, TransportError, TransportEvent, TransportInput, TransportOutput, TransportState},
};
use media_server_protocol::{
    endpoint::{PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackPriority, TrackSource},
    media::{MediaKind, MediaMeta, MediaPacket},
    transport::{RpcError, RpcResult},
};
use media_server_utils::Count;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    collections::DynamicDeque,
    return_if_none, TaskSwitcherChild,
};
use sdp_rs::SessionDescription;

use crate::RtpEngineError;

const TIMEOUT_DURATION_MS: u64 = 60_000;

const REMOTE_AUDIO_TRACK: RemoteTrackId = RemoteTrackId::build(0);
const LOCAL_AUDIO_TRACK: LocalTrackId = LocalTrackId::build(0);
const AUDIO_NAME: &str = "audio_main";
const DEFAULT_PRIORITY: TrackPriority = TrackPriority::build(1);

#[allow(clippy::large_enum_variant)]
pub enum ExtIn {
    SetAnswer(u64, String),
    Disconnect(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtOut {
    SetAnswer(u64, RpcResult<()>),
    Disconnect(u64),
}

pub struct TransportRtpEngine {
    _c: Count<Self>,
    remote: Option<SocketAddr>,
    room: RoomId,
    peer: PeerId,
    udp_slot: Option<usize>,
    answered: bool,
    connected: bool,
    created: Instant,
    last_recv_rtp: Option<Instant>,
    last_send_rtp: Option<Instant>,
    queue: DynamicDeque<TransportOutput<ExtOut>, 4>,
    pcma_to_opus: AudioTranscoder<PcmaDecoder, OpusEncoder>,
    opus_to_pcma: AudioTranscoder<OpusDecoder, PcmaEncoder>,
    tmp_buf: [u8; 1500],
    shutdown: bool,
}

impl TransportRtpEngine {
    pub fn new_offer(room: RoomId, peer: PeerId, ip: IpAddr) -> Result<(Self, String), String> {
        let socket = std::net::UdpSocket::bind(SocketAddr::new(ip, 0)).map_err(|e| e.to_string())?;
        let port = socket.local_addr().map_err(|e| e.to_string())?.port();
        let answer = sdp_builder(ip, port);

        Ok((
            Self {
                _c: Default::default(),
                remote: None,
                room,
                peer,
                udp_slot: None,
                answered: false,
                connected: false,
                created: Instant::now(),
                last_recv_rtp: None,
                last_send_rtp: None,
                queue: DynamicDeque::from([
                    TransportOutput::Net(BackendOutgoing::UdpListen {
                        addr: SocketAddr::new(ip, port),
                        reuse: false,
                    }),
                    TransportOutput::Event(TransportEvent::State(TransportState::New)),
                ]),
                pcma_to_opus: AudioTranscoder::new(PcmaDecoder::default(), OpusEncoder::default()),
                opus_to_pcma: AudioTranscoder::new(OpusDecoder::default(), PcmaEncoder::default()),
                tmp_buf: [0; 1500],
                shutdown: false,
            },
            answer,
        ))
    }

    pub fn new_answer(room: RoomId, peer: PeerId, ip: IpAddr, offer: &str) -> Result<(Self, String), String> {
        let mut offer = SessionDescription::try_from(offer.to_string()).map_err(|e| e.to_string())?;
        let dest_ip: IpAddr = if let Some(conn) = offer.connection {
            conn.connection_address.base
        } else {
            offer.origin.unicast_address
        };
        let dest_port = offer.media_descriptions.pop().ok_or("MEDIA_NOT_FOUND".to_string())?.media.port;
        let remote = SocketAddr::new(dest_ip, dest_port);

        log::info!("[TransportRtpEngine] on create answer => set remote to {remote}");

        let socket = std::net::UdpSocket::bind(SocketAddr::new(ip, 0)).map_err(|e| e.to_string())?;
        let port = socket.local_addr().map_err(|e| e.to_string())?.port();
        let answer = sdp_builder(ip, port);

        Ok((
            Self {
                _c: Default::default(),
                remote: Some(remote),
                room,
                peer,
                udp_slot: None,
                answered: true,
                connected: false,
                created: Instant::now(),
                last_recv_rtp: None,
                last_send_rtp: None,
                queue: DynamicDeque::from([
                    TransportOutput::Net(BackendOutgoing::UdpListen {
                        addr: SocketAddr::new(ip, port),
                        reuse: false,
                    }),
                    TransportOutput::Event(TransportEvent::State(TransportState::Connecting(dest_ip))),
                ]),
                pcma_to_opus: AudioTranscoder::new(PcmaDecoder::default(), OpusEncoder::default()),
                opus_to_pcma: AudioTranscoder::new(OpusDecoder::default(), PcmaEncoder::default()),
                tmp_buf: [0; 1500],
                shutdown: false,
            },
            answer,
        ))
    }

    fn set_answer(&mut self, answer: &str) -> RpcResult<()> {
        let mut answer = SessionDescription::try_from(answer.to_string()).map_err(|e| RpcError::new(RtpEngineError::InvalidSdp as u32, &e.to_string()))?;
        log::info!("[TransportRtpEngine] on answer {answer:?}");
        let dest_ip: IpAddr = if let Some(conn) = answer.connection {
            conn.connection_address.base
        } else {
            answer.origin.unicast_address
        };
        let dest_port = answer.media_descriptions.pop().ok_or(RpcError::new2(RtpEngineError::SdpMediaNotFound))?.media.port;
        let remote = SocketAddr::new(dest_ip, dest_port);
        self.remote = Some(remote);
        self.answered = true;
        log::info!("[TransportRtpEngine] on answer => reset remote to {remote}");
        self.queue.push_back(TransportOutput::Event(TransportEvent::State(TransportState::Connecting(dest_ip))));
        Ok(())
    }
}

impl Transport<ExtIn, ExtOut> for TransportRtpEngine {
    fn on_tick(&mut self, _now: Instant) {
        if !self.shutdown {
            let last_activity = match (self.last_recv_rtp, self.last_send_rtp) {
                (None, None) => self.created,
                (Some(_time), None) => self.created, //we need two way, if only one-way => disconnect
                (None, Some(_time)) => self.created, //we need two way, if only one-way => disconnect
                (Some(time1), Some(time2)) => time1.max(time2),
            };

            if last_activity.elapsed() >= Duration::from_millis(TIMEOUT_DURATION_MS) {
                log::warn!("[TransportRtpEngine] timeout after {TIMEOUT_DURATION_MS} ms don't has activity");
                self.queue
                    .push_back(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(Some(TransportError::Timeout)))));
                self.shutdown = true;
            }
        }
    }

    fn on_input(&mut self, now: Instant, input: TransportInput<ExtIn>) {
        match input {
            TransportInput::Net(event) => self.on_backend(now, event),
            TransportInput::Endpoint(event) => self.on_event(now, event),
            TransportInput::RpcRes(_, res) => {
                log::info!("[TransportRtpEngine] on rpc_res {res:?}");
            }
            TransportInput::Ext(ext) => match ext {
                ExtIn::SetAnswer(req_id, sdp) => {
                    log::info!("[TransportRtpEngine] received answer from client");
                    let res = self.set_answer(&sdp);
                    if let Err(e) = &res {
                        log::error!("[TransportRtpEngine] set answer from client error {e:?}");
                    }
                    self.queue.push_back(TransportOutput::Ext(ExtOut::SetAnswer(req_id, res)));
                }
                ExtIn::Disconnect(req_id) => {
                    log::info!("[TransportRtpEngine] switched to disconnected with close action from client");
                    self.queue.push_back(TransportOutput::Ext(ExtOut::Disconnect(req_id)));
                    self.queue.push_back(TransportOutput::Event(TransportEvent::State(TransportState::Disconnected(None))));
                }
            },
        }
    }

    fn on_shutdown(&mut self, _now: Instant) {
        if !self.shutdown {
            log::info!("[TransportRtpEngine] shutdown request");
            self.shutdown = true;
        }
    }
}

impl TransportRtpEngine {
    fn on_backend(&mut self, now: Instant, event: BackendIncoming) {
        match event {
            BackendIncoming::UdpListenResult { bind, result } => match result {
                Ok((addr, slot)) => {
                    log::info!("[TransportRtpEngine] bind {bind} => {addr} with slot {slot}");
                    self.udp_slot = Some(slot);
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
                        self.last_recv_rtp = Some(now);
                        log::debug!(
                            "[TransportRtpEngine] on rtp from {} {} {:?} {} len {}",
                            from,
                            rtp.payload_type(),
                            rtp.sequence_number(),
                            rtp.timestamp(),
                            rtp.payload().len()
                        );
                        if rtp.payload_type() == 8 {
                            if self.answered && !self.connected {
                                self.connected = true;
                                log::info!("[TransportRtpEngine] first rtp packet after answered => switch to connected mode and join room");
                                self.queue.push_back(TransportOutput::Event(TransportEvent::State(TransportState::Connected(from.ip()))));
                                self.queue.push_back(TransportOutput::Event(TransportEvent::RemoteTrack(
                                    REMOTE_AUDIO_TRACK,
                                    RemoteTrackEvent::Started {
                                        name: AUDIO_NAME.to_string(),
                                        priority: TrackPriority::from(100),
                                        meta: TrackMeta::default_audio(),
                                    },
                                )));
                                self.queue
                                    .push_back(TransportOutput::Event(TransportEvent::LocalTrack(LOCAL_AUDIO_TRACK, LocalTrackEvent::Started(MediaKind::Audio))));
                                self.queue.push_back(TransportOutput::RpcReq(
                                    0.into(),
                                    EndpointReq::JoinRoom(
                                        self.room.clone(),
                                        self.peer.clone(),
                                        PeerMeta { metadata: None, extra_data: None },
                                        RoomInfoPublish { peer: true, tracks: true },
                                        RoomInfoSubscribe { peers: false, tracks: true },
                                        None,
                                    ),
                                ));
                            }

                            //TODO avoid hard-coding
                            if let Some(size) = self.pcma_to_opus.transcode(rtp.payload(), &mut self.tmp_buf) {
                                let media = MediaPacket {
                                    ts: rtp.timestamp().wrapping_mul(6), //TODO avoid overflow
                                    seq: rtp.sequence_number().into(),
                                    marker: rtp.mark(),
                                    nackable: false,
                                    layers: None,
                                    meta: MediaMeta::Opus { audio_level: Some(0) }, //TODO how to get audio level from opus?
                                    data: self.tmp_buf[..size].to_vec(),
                                };
                                log::debug!("[TransportRtpEngine] transcode to opus {} {} {}", media.seq, media.ts, media.data.len());
                                self.queue
                                    .push_back(TransportOutput::Event(TransportEvent::RemoteTrack(REMOTE_AUDIO_TRACK, RemoteTrackEvent::Media(media))));
                            }
                        }
                    }
                }
            }
        }
    }

    fn on_event(&mut self, now: Instant, event: EndpointEvent) {
        match event {
            EndpointEvent::PeerTrackStarted(peer, track, _) => {
                //TODO select only one or audio_mixer
                if self.peer != peer {
                    log::info!("[TransportRtpEngine] room {} peer {} attach to {peer}/{track}", self.room, self.peer);
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
                    self.last_send_rtp = Some(now);
                    let slot = return_if_none!(self.udp_slot);
                    if let Some(remote) = self.remote {
                        if let Some(size) = self.opus_to_pcma.transcode(&media.data, &mut self.tmp_buf) {
                            log::debug!(
                                "[TransportRtpEngine] transcode opus rtp to pcma {} {} {} len {} => {}",
                                remote,
                                media.seq,
                                media.ts,
                                media.data.len(),
                                size
                            );
                            if let Ok(data) = rtp_rs::RtpPacketBuilder::new()
                                .marked(media.marker)
                                .payload_type(8) // TODO avoid hard-coding
                                .timestamp(media.ts / 6) //TODO avoid hard-coding downsample
                                .sequence(media.seq.into())
                                .payload(&self.tmp_buf[..size])
                                .build()
                            {
                                self.queue.push_back(TransportOutput::Net(BackendOutgoing::UdpPacket { slot, to: remote, data: data.into() }))
                            }
                        } else {
                            log::warn!("[TransportRtpEngine] send rtp without remote addr");
                        }
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

    fn is_empty(&self) -> bool {
        self.shutdown && self.queue.is_empty()
    }

    fn empty_event(&self) -> TransportOutput<ExtOut> {
        TransportOutput::OnResourceEmpty
    }

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

    if (128..192).contains(&byte0) && len > 2 {
        let byte1 = value[1];
        let payload_type = byte1 & 0x7F;
        Some(if payload_type < 64 {
            MultiplexKind::Rtp
        } else if (64..96).contains(&payload_type) {
            MultiplexKind::Rtcp
        } else {
            MultiplexKind::Rtp
        })
    } else {
        None
    }
}

//TODO adaptive codec type
fn sdp_builder(ip: IpAddr, port: u16) -> String {
    format!(
        "v=0
o=Z 0 1094063179 IN IP4 {ip}
s=Z
c=IN IP4 {ip}
t=0 0
m=audio {port} RTP/AVP 8 101
a=rtpmap:8 PCMA/8000
a=rtpmap:101 telephone-event/8000
a=fmtp:101 0-16
a=sendrecv
a=rtcp-mux
"
    )
}
