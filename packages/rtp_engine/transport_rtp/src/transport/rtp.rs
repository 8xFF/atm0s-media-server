use std::{collections::VecDeque, io, net::IpAddr, time::Instant};

use media_server_core::{
    endpoint::{EndpointEvent, EndpointLocalTrackConfig, EndpointLocalTrackEvent, EndpointLocalTrackReq, EndpointReq},
    transport::{LocalTrackId, RemoteTrackEvent, RemoteTrackId, TransportEvent, TransportOutput},
};
use media_server_protocol::{
    endpoint::{BitrateControlMode, PeerId, PeerMeta, RoomId, RoomInfoPublish, RoomInfoSubscribe, TrackMeta, TrackName, TrackPriority, TrackSource},
    media::{MediaKind, MediaPacket, MediaScaling},
    protobuf::cluster_connector::peer_event::LocalTrack,
};
use rtp::packet::Packet;
use webrtc_util::Unmarshal;

use crate::packets::MultiplexKind;

use super::{InternalNetInput, InternalOutput, TransportRtpInternal};

const AUDIO_REMOVE_TRACK: RemoteTrackId = RemoteTrackId(0);
const AUDIO_LOCAL_TRACK: LocalTrackId = LocalTrackId(0);
const AUDIO_NAME: &str = "audio_main";
const DEFAULT_PRIORITY: TrackPriority = TrackPriority(1);

#[derive(Default, Debug)]
struct SubscribeStreams {
    peer: Option<PeerId>,
    audio: Option<TrackName>,
}

pub struct RtpInternal {
    remote: IpAddr,
    room: RoomId,
    peer: PeerId,
    subscribed: SubscribeStreams,
    queue: VecDeque<InternalOutput>,
}

impl RtpInternal {
    pub fn new(remote: IpAddr, room: RoomId, peer: PeerId) -> Self {
        Self {
            remote,
            room: room.clone(),
            peer: peer.clone(),
            subscribed: SubscribeStreams::default(),
            queue: VecDeque::from(vec![
                InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(),
                    EndpointReq::JoinRoom(
                        room.clone(),
                        peer.clone(),
                        PeerMeta { metadata: None },
                        RoomInfoPublish { peer: true, tracks: true },
                        RoomInfoSubscribe { peers: false, tracks: false },
                        None,
                    ),
                )),
                InternalOutput::TransportOutput(TransportOutput::Event(TransportEvent::RemoteTrack(
                    AUDIO_REMOVE_TRACK,
                    RemoteTrackEvent::Started {
                        name: AUDIO_NAME.to_string(),
                        meta: TrackMeta {
                            kind: MediaKind::Audio,
                            scaling: MediaScaling::None,
                            control: BitrateControlMode::MaxBitrate,
                            metadata: None,
                        },
                        priority: TrackPriority(1),
                    },
                ))),
            ]),
        }
    }
}

impl RtpInternal {}

impl TransportRtpInternal for RtpInternal {
    fn on_tick(&mut self, _now: Instant) {}

    fn on_endpoint_event(&mut self, _now: Instant, event: media_server_core::endpoint::EndpointEvent) {
        match event {
            EndpointEvent::PeerTrackStarted(peer, track, meta) => {
                if !meta.kind.is_audio() {
                    return;
                }
                self.try_subscribe(peer, track, meta);
            }
            EndpointEvent::LocalMediaTrack(_track, event) => match event {
                EndpointLocalTrackEvent::Media(pkt) => {}
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_input(&mut self, input: InternalNetInput) -> Result<(), io::Error> {
        match MultiplexKind::try_from(input.data) {
            Ok(MultiplexKind::Rtp) => {
                let mut buf = input.data;
                let rtp_packet = Packet::unmarshal(&mut buf);
                match rtp_packet {
                    Ok(packet) => {
                        log::trace!("[RtpTransportInternal] got a rtp packet {:?}", packet.header);
                        Ok(())
                    }
                    Err(e) => {
                        log::error!("[RtpTransportInternal] error parsing rtp packet {:?}", e);
                        Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
                    }
                }
            }
            Ok(MultiplexKind::Rtcp) => {
                log::trace!("[RtpTransportInternal] got a rtcp packet");
                Ok(())
            }
            Err(e) => {
                log::error!("[RtpTransportInternal] unknown packet {:?}", e);
                Err(e)
            }
        }
    }

    fn pop_output(&mut self, now: std::time::Instant) -> Option<super::InternalOutput> {
        self.queue.pop_front()
    }
}

impl RtpInternal {
    fn try_subscribe(&mut self, peer: PeerId, track: TrackName, _meta: TrackMeta) {
        log::info!("[RtpTransportInternal] try subscribe {peer} {track}");
        if self.subscribed.peer.is_none() || self.subscribed.peer.eq(&Some(peer.clone())) {
            self.subscribed.peer = Some(peer.clone());
            self.subscribed.audio = Some(track.clone());
            log::info!("[RtpTransportInternal] send subscribe {peer} {track}");
            self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                0.into(),
                EndpointReq::LocalTrack(
                    AUDIO_LOCAL_TRACK,
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
            )))
        }
    }

    fn try_unsubscribe(&mut self, peer: PeerId, track: TrackName) {
        log::info!("[RtpTransportInternal] try unsubcribe {peer} {track}");
        if self.subscribed.peer.eq(&Some(peer.clone())) {
            if self.subscribed.audio.eq(&(Some(track.clone()))) {
                self.subscribed.audio = None;
                log::info!("[RtpTransportInternal] send unsubcribe {peer} {track}");
                self.queue.push_back(InternalOutput::TransportOutput(TransportOutput::RpcReq(
                    0.into(),
                    EndpointReq::LocalTrack(AUDIO_LOCAL_TRACK, EndpointLocalTrackReq::Detach()),
                )))
            }

            if self.subscribed.audio.is_none() {
                self.subscribed.peer = None;
            }
        }
    }
}
