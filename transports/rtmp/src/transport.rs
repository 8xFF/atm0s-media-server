use std::collections::VecDeque;

use crate::rtmp::{audio_convert::RtmpAacToMediaPacketOpus, video_convert::RtmpH264ToMediaPacketH264, RtmpSession, ServerEvent};
use async_std::net::TcpStream;
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{AsyncReadExt, AsyncWriteExt};
use media_utils::ErrorDebugger;
use rml_rtmp::sessions::ServerSessionError;
use transport::{
    MediaKind, MediaSampleRate, RemoteTrackIncomingEvent, TrackMeta, Transport, TransportError, TransportIncomingEvent, TransportOutgoingEvent, TransportRuntimeError, TransportStateEvent,
};

const AUDIO_TRACK_ID: u16 = 0;
const VIDEO_TRACK_ID: u16 = 1;

type RmIn = EndpointRpcIn;
type RrIn = RemoteTrackRpcIn;
type RlIn = LocalTrackRpcIn;
type RmOut = EndpointRpcOut;
type RrOut = RemoteTrackRpcOut;
type RlOut = LocalTrackRpcOut;

pub struct RtmpTransport {
    socket: TcpStream,
    session: RtmpSession,
    buf: Vec<u8>,
    room: Option<String>,
    peer: Option<String>,
    actions: VecDeque<TransportIncomingEvent<RmIn, RrIn, RlIn>>,
    audio_convert: RtmpAacToMediaPacketOpus,
    video_convert: RtmpH264ToMediaPacketH264,
}

impl RtmpTransport {
    pub fn new(socket: TcpStream) -> Self {
        Self {
            socket,
            session: RtmpSession::new(),
            buf: vec![0; 1 << 12],
            room: None,
            peer: None,
            actions: VecDeque::new(),
            audio_convert: RtmpAacToMediaPacketOpus::new(),
            video_convert: RtmpH264ToMediaPacketH264::new(),
        }
    }

    pub fn room(&self) -> Option<String> {
        self.room.clone()
    }

    pub fn peer(&self) -> Option<String> {
        self.peer.clone()
    }
}

#[async_trait::async_trait]
impl Transport<(), RmIn, RrIn, RlIn, RmOut, RrOut, RlOut> for RtmpTransport {
    fn on_tick(&mut self, _now_ms: u64) -> Result<(), TransportError> {
        Ok(())
    }
    fn on_event(&mut self, _now_ms: u64, _event: TransportOutgoingEvent<RmOut, RrOut, RlOut>) -> Result<(), TransportError> {
        Ok(())
    }
    fn on_custom_event(&mut self, _now_ms: u64, _event: ()) -> Result<(), TransportError> {
        Ok(())
    }
    async fn recv(&mut self, _now_ms: u64) -> Result<TransportIncomingEvent<RmIn, RrIn, RlIn>, TransportError> {
        if let Some(event) = self.actions.pop_front() {
            return Ok(event);
        }
        while let Some(action) = self.session.pop_action() {
            match action {
                ServerEvent::OutboundPacket(pkt) => {
                    self.socket.write(&pkt.bytes).await.map_err(|_e| TransportError::NetworkError)?;
                }
                ServerEvent::ConnectionRequested { request_id, .. } => {
                    self.session
                        .on_accept_request(request_id)
                        .map_err(|_e| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
                }
                ServerEvent::PublishRequest { request_id, app_name, stream_key } => {
                    //TODO check if app_name and stream_key is valid
                    self.session
                        .on_accept_request(request_id)
                        .map_err(|_e: ServerSessionError| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
                    self.room = Some(app_name);
                    self.peer = Some(stream_key);

                    // Because we need to wait connect to get room and peer info.
                    // TODO dont use this trick
                    self.actions.push_back(TransportIncomingEvent::State(TransportStateEvent::Connected));
                    self.actions.push_back(TransportIncomingEvent::RemoteTrackAdded(
                        "audio_main".to_string(),
                        0,
                        TrackMeta::new(MediaKind::Audio, MediaSampleRate::Hz48000, None),
                    ));
                    self.actions.push_back(TransportIncomingEvent::RemoteTrackAdded(
                        "video_main".to_string(),
                        1,
                        TrackMeta::new(MediaKind::Video, MediaSampleRate::Hz90000, None),
                    ));

                    return Ok(TransportIncomingEvent::State(TransportStateEvent::Connected));
                }
                ServerEvent::PublishData { kind, data, ts_ms } => match kind {
                    MediaKind::Audio => {
                        if self.audio_convert.push(data, ts_ms).is_some() {
                            while let Some(pkt) = self.audio_convert.pop() {
                                self.actions
                                    .push_back(TransportIncomingEvent::RemoteTrackEvent(AUDIO_TRACK_ID, RemoteTrackIncomingEvent::MediaPacket(pkt)));
                            }
                        }
                    }
                    MediaKind::Video => {
                        if self.video_convert.push(data, ts_ms).is_some() {
                            while let Some(pkt) = self.video_convert.pop() {
                                self.actions
                                    .push_back(TransportIncomingEvent::RemoteTrackEvent(VIDEO_TRACK_ID, RemoteTrackIncomingEvent::MediaPacket(pkt)));
                            }
                        }
                    }
                },
                ServerEvent::PublishFinished { .. } => {
                    self.actions.push_back(TransportIncomingEvent::RemoteTrackRemoved("audio_main".to_string(), 0));
                    self.actions.push_back(TransportIncomingEvent::RemoteTrackRemoved("video_main".to_string(), 1));
                    self.actions.push_back(TransportIncomingEvent::State(TransportStateEvent::Disconnected));
                    return Ok(TransportIncomingEvent::Continue);
                }
            }
        }

        let len = self.socket.read(&mut self.buf).await.map_err(|_e| TransportError::NetworkError)?;
        if len == 0 {
            self.actions.push_back(TransportIncomingEvent::State(TransportStateEvent::Disconnected));
        } else {
            self.session
                .on_network(&self.buf[..len])
                .map_err(|_e| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        }
        Ok(TransportIncomingEvent::Continue)
    }

    async fn close(&mut self) {
        self.socket.close().await.log_error("Should close socket");
        self.actions.push_back(TransportIncomingEvent::State(TransportStateEvent::Disconnected));
    }
}
