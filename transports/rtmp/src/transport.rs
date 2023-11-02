use std::collections::VecDeque;

use crate::rtmp::{RtmpSession, ServerEvent};
use async_std::net::TcpStream;
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{AsyncReadExt, AsyncWriteExt};
use rml_rtmp::sessions::ServerSessionError;
use transport::{Transport, TransportError, TransportIncomingEvent, TransportOutgoingEvent, TransportRuntimeError, TransportStateEvent};

pub struct RtmpTransport<RmIn, RrIn, RlIn> {
    socket: TcpStream,
    session: RtmpSession,
    buf: Vec<u8>,
    room: Option<String>,
    peer: Option<String>,
    actions: VecDeque<TransportIncomingEvent<RmIn, RrIn, RlIn>>,
}

impl<RmIn, RrIn, RlIn> RtmpTransport<RmIn, RrIn, RlIn> {
    pub fn new(socket: TcpStream) -> Self {
        Self {
            socket,
            session: RtmpSession::new(),
            buf: vec![0; 1 << 12],
            room: None,
            peer: None,
            actions: VecDeque::new(),
        }
    }

    pub fn room(&self) -> Option<String> {
        self.room.clone()
    }

    pub fn peer(&self) -> Option<String> {
        self.peer.clone()
    }
}

type RmIn = EndpointRpcIn;
type RrIn = RemoteTrackRpcIn;
type RlIn = LocalTrackRpcIn;
type RmOut = EndpointRpcOut;
type RrOut = RemoteTrackRpcOut;
type RlOut = LocalTrackRpcOut;

#[async_trait::async_trait]
impl Transport<(), RmIn, RrIn, RlIn, RmOut, RrOut, RlOut> for RtmpTransport<RmIn, RrIn, RlIn> {
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        Ok(())
    }
    fn on_event(&mut self, now_ms: u64, event: TransportOutgoingEvent<RmOut, RrOut, RlOut>) -> Result<(), TransportError> {
        Ok(())
    }
    fn on_custom_event(&mut self, now_ms: u64, event: ()) -> Result<(), TransportError> {
        Ok(())
    }
    async fn recv(&mut self, now_ms: u64) -> Result<TransportIncomingEvent<RmIn, RrIn, RlIn>, TransportError> {
        if let Some(event) = self.actions.pop_front() {
            return Ok(event);
        }
        while let Some(action) = self.session.pop_action() {
            match action {
                ServerEvent::DisconnectConnection => {
                    return Ok(TransportIncomingEvent::State(TransportStateEvent::Disconnected));
                }
                ServerEvent::OutboundPacket(pkt) => {
                    self.socket.write(&pkt.bytes).await.map_err(|e| TransportError::NetworkError)?;
                }
                ServerEvent::ConnectionRequested { request_id, app_name } => {
                    self.session
                        .on_accept_request(request_id)
                        .map_err(|e| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
                }
                ServerEvent::PublishRequest { request_id, app_name, stream_key } => {
                    //TODO check if app_name and stream_key is valid
                    self.session
                        .on_accept_request(request_id)
                        .map_err(|e: ServerSessionError| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
                    self.room = Some(app_name);
                    self.peer = Some(stream_key);

                    // Because we need to wait connect to get room and peer info.
                    // TODO dont use this trick
                    self.actions.push_back(TransportIncomingEvent::State(TransportStateEvent::Connected));

                    return Ok(TransportIncomingEvent::State(TransportStateEvent::Connected));
                }
                ServerEvent::PublishFinished { app_name, stream_key } => {}
            }
        }

        let len = self.socket.read(&mut self.buf).await.map_err(|e| TransportError::NetworkError)?;
        if len == 0 {
            return Ok(TransportIncomingEvent::State(TransportStateEvent::Disconnected));
        }

        self.session
            .on_network(&self.buf[..len])
            .map_err(|_e| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        Ok(TransportIncomingEvent::Continue)
    }

    async fn close(&mut self) {
        self.socket.close().await;
    }
}
