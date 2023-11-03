use std::collections::VecDeque;

use bytes::Bytes;
use rml_rtmp::{
    chunk_io::Packet,
    handshake::{Handshake, HandshakeProcessResult, PeerType},
    sessions::{PublishMode, ServerSession, ServerSessionConfig, ServerSessionError, ServerSessionEvent, ServerSessionResult},
};
use transport::MediaKind;

pub(crate) mod audio_convert;
pub(crate) mod video_convert;

#[derive(Debug)]
pub enum ServerEvent {
    OutboundPacket(Packet),
    ConnectionRequested { request_id: u32, app_name: String },
    PublishRequest { request_id: u32, app_name: String, stream_key: String },
    PublishData { kind: MediaKind, data: Bytes, ts_ms: u32 },
    PublishFinished { app_name: String, stream_key: String },
}

pub struct RtmpSession {
    handshake: Option<Handshake>,
    session: ServerSession,
    queue_internal: VecDeque<ServerSessionResult>,
    outputs: VecDeque<ServerEvent>,
}

impl RtmpSession {
    pub fn new() -> Self {
        let config = ServerSessionConfig::new();
        let (session, queue_internal) = ServerSession::new(config).expect("Should create server session");

        Self {
            handshake: Some(Handshake::new(PeerType::Server)),
            session,
            queue_internal: queue_internal.into(),
            outputs: Default::default(),
        }
    }

    pub fn on_accept_request(&mut self, req_id: u32) -> Result<(), ServerSessionError> {
        log::info!("[ServerSession] accepted request {req_id}");
        let results = self.session.accept_request(req_id)?;
        self.queue_internal.extend(results);
        self.pop_internal_queue()?;
        Ok(())
    }

    pub fn on_reject_request(&mut self, req_id: u32, code: &str, desc: &str) -> Result<(), ServerSessionError> {
        log::info!("[ServerSession] rejected request {req_id} {code} {desc}");
        let results = self.session.reject_request(req_id, code, desc)?;
        self.queue_internal.extend(results);
        self.pop_internal_queue()?;
        Ok(())
    }

    pub fn on_network(&mut self, data: &[u8]) -> Result<(), ServerSessionError> {
        if let Some(handshake) = self.handshake.as_mut() {
            log::info!("[ServerSession] handshake process bytes: {}", data.len());
            match handshake.process_bytes(data).unwrap() {
                HandshakeProcessResult::InProgress { response_bytes } => {
                    log::info!("[ServerSession] handshake process in progress send bytes {}", response_bytes.len());
                    self.outputs.push_back(ServerEvent::OutboundPacket(Packet {
                        bytes: response_bytes,
                        can_be_dropped: false,
                    }));
                    return Ok(());
                }
                HandshakeProcessResult::Completed { response_bytes, remaining_bytes } => {
                    log::info!("[ServerSession] handshake process completed send bytes {}, remain {}", response_bytes.len(), remaining_bytes.len());
                    self.outputs.push_back(ServerEvent::OutboundPacket(Packet {
                        bytes: response_bytes,
                        can_be_dropped: false,
                    }));
                    self.handshake = None;
                    if remaining_bytes.len() > 0 {
                        return self.on_network(&remaining_bytes);
                    } else {
                        self.pop_internal_queue()?;
                        return Ok(());
                    }
                }
            }
        }

        let results = self.session.handle_input(data)?;
        self.queue_internal.extend(results);
        self.pop_internal_queue()?;
        Ok(())
    }

    pub fn pop_action(&mut self) -> Option<ServerEvent> {
        self.outputs.pop_front()
    }

    fn pop_internal_queue(&mut self) -> Result<(), ServerSessionError> {
        while let Some(result) = self.queue_internal.pop_front() {
            match result {
                ServerSessionResult::OutboundResponse(packet) => {
                    self.outputs.push_back(ServerEvent::OutboundPacket(packet));
                }

                ServerSessionResult::RaisedEvent(event) => {
                    self.handle_raised_event(event)?;
                }

                x => log::info!("Server result received: {:?}", x),
            }
        }

        Ok(())
    }

    fn handle_raised_event(&mut self, event: ServerSessionEvent) -> Result<(), ServerSessionError> {
        match event {
            ServerSessionEvent::ConnectionRequested { request_id, app_name } => {
                log::info!("[RtmpSession] Connection requested: {:?} {:?}", request_id, app_name);
                self.outputs.push_back(ServerEvent::ConnectionRequested { request_id, app_name });
            }
            ServerSessionEvent::PublishStreamRequested {
                request_id,
                app_name,
                stream_key,
                mode,
            } => {
                if matches!(mode, PublishMode::Live) {
                    log::info!("[RtmpSession] Publish stream requested: {} {} {}", request_id, app_name, stream_key);
                    self.outputs.push_back(ServerEvent::PublishRequest { request_id, app_name, stream_key });
                } else {
                    let results = self.session.reject_request(request_id, "NetStream.Publish.BadName", "Publishing only supported live mode")?;
                    self.queue_internal.extend(results);
                }
            }
            ServerSessionEvent::ReleaseStreamRequested { request_id, app_name, stream_key } => {
                log::info!("[RtmpSession] Release stream requested: {} {} {}", request_id, app_name, stream_key);
                let results = self.session.accept_request(request_id)?;
                self.queue_internal.extend(results);
            }
            ServerSessionEvent::PublishStreamFinished { app_name, stream_key } => {
                log::info!("[RtmpSession] Released stream: {} {}", app_name, stream_key);
                self.outputs.push_back(ServerEvent::PublishFinished { app_name, stream_key });
            }
            ServerSessionEvent::StreamMetadataChanged { app_name, stream_key, metadata } => {
                log::info!("[RtmpSession] Stream metadata changed: {} {} {:?}", app_name, stream_key, metadata);
            }
            ServerSessionEvent::AudioDataReceived {
                app_name,
                stream_key,
                data,
                timestamp,
            } => {
                //TODO custom rml_rtmp for avoid stream_key each time
                log::debug!("[RtmpSession] Audio data received: {} {} {:?}", app_name, stream_key, timestamp);
                self.outputs.push_back(ServerEvent::PublishData {
                    kind: MediaKind::Audio,
                    data,
                    ts_ms: timestamp.value,
                });
            }
            ServerSessionEvent::VideoDataReceived {
                app_name,
                stream_key,
                data,
                timestamp,
            } => {
                //TODO custom rml_rtmp for avoid stream_key each time
                log::debug!("[RtmpSession] Video video received: {:?} {:?} {:?}", app_name, stream_key, timestamp);
                self.outputs.push_back(ServerEvent::PublishData {
                    kind: MediaKind::Video,
                    data,
                    ts_ms: timestamp.value,
                });
            }
            ServerSessionEvent::PlayStreamRequested { request_id, .. } => {
                //auto reject
                self.session.reject_request(request_id, "NetStream.Play.StreamNotFound", "No such stream")?;
            }
            ServerSessionEvent::PlayStreamFinished { .. } => {}
            ServerSessionEvent::AcknowledgementReceived { .. } => {}
            ServerSessionEvent::PingResponseReceived { .. } => {}
            ServerSessionEvent::ClientChunkSizeChanged { .. } => {}
            ServerSessionEvent::UnhandleableAmf0Command { .. } => {}
        }

        Ok(())
    }
}
