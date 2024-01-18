use std::{collections::VecDeque, net::SocketAddr};

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{select, FutureExt};
use media_utils::ErrorDebugger;
use rsip::{
    typed::{Contact, ContentType, MediaType},
    Auth, Host, HostWithPort, Param, Uri,
};
use transport::{
    LocalTrackOutgoingEvent, MediaKind, MediaSampleRate, RemoteTrackIncomingEvent, TrackMeta, Transport, TransportError, TransportIncomingEvent, TransportOutgoingEvent, TransportRuntimeError,
    TransportStateEvent,
};

use crate::{
    processor::{call_in::CallInProcessor, Processor, ProcessorAction},
    rtp_engine::{RtpEngine, RtpEngineError},
    sip_request::SipRequest,
    virtual_socket::VirtualSocket,
    GroupId, SipMessage, LOCAL_TRACK_AUDIO_MAIN, REMOTE_TRACK_AUDIO_MAIN,
};

type RmIn = EndpointRpcIn;
type RrIn = RemoteTrackRpcIn;
type RlIn = LocalTrackRpcIn;
type RmOut = EndpointRpcOut;
type RrOut = RemoteTrackRpcOut;
type RlOut = LocalTrackRpcOut;

pub struct SipTransportIn {
    rtp_engine: RtpEngine,
    socket: VirtualSocket<GroupId, SipMessage>,
    logic: CallInProcessor,
    actions: VecDeque<TransportIncomingEvent<RmIn, RrIn, RlIn>>,
}

impl SipTransportIn {
    pub async fn new(now_ms: u64, bind_addr: SocketAddr, socket: VirtualSocket<GroupId, SipMessage>, req: SipRequest) -> Result<Self, RtpEngineError> {
        let local_contact = Contact {
            uri: Uri {
                auth: req.to.uri.user().map(|u| Auth { user: u.to_string(), password: None }),
                scheme: Some(rsip::Scheme::Sip),
                host_with_port: HostWithPort {
                    host: Host::IpAddr(bind_addr.ip()),
                    port: Some(bind_addr.port().into()),
                },
                params: vec![Param::Transport(rsip::Transport::Udp)],
                ..Default::default()
            },
            display_name: None,
            params: vec![],
        };
        let mut rtp_engine = RtpEngine::new(bind_addr.ip()).await;
        log::info!("Create transport {}", req.body_str());
        rtp_engine.process_remote_sdp(&req.body_str()).await?;
        Ok(Self {
            rtp_engine,
            socket,
            logic: CallInProcessor::new(now_ms, local_contact, req),
            actions: VecDeque::new(),
        })
    }

    pub fn ringing(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.logic.ringing(now_ms).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        Ok(())
    }

    pub fn accept(&mut self, now_ms: u64) -> Result<(), TransportError> {
        let local_sdp = self.rtp_engine.create_local_sdp();
        log::info!("Will accept now, local_sdp: {}", local_sdp);
        self.logic
            .accept(now_ms, Some((ContentType(MediaType::Sdp(vec![])).into(), local_sdp.as_bytes().to_vec())))
            .map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        self.actions.push_back(TransportIncomingEvent::State(TransportStateEvent::Connected));
        self.actions.push_back(TransportIncomingEvent::RemoteTrackAdded(
            "audio_main".to_string(),
            REMOTE_TRACK_AUDIO_MAIN,
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        ));
        self.actions.push_back(TransportIncomingEvent::LocalTrackAdded(
            "audio_0".to_string(),
            LOCAL_TRACK_AUDIO_MAIN,
            TrackMeta {
                kind: MediaKind::Audio,
                sample_rate: MediaSampleRate::Hz48000,
                label: None,
            },
        ));
        Ok(())
    }

    pub fn reject(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.logic.reject(now_ms).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Transport<(), RmIn, RrIn, RlIn, RmOut, RrOut, RlOut> for SipTransportIn {
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.logic.on_tick(now_ms).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        Ok(())
    }

    fn on_event(&mut self, now_ms: u64, event: TransportOutgoingEvent<RmOut, RrOut, RlOut>) -> Result<(), TransportError> {
        match event {
            TransportOutgoingEvent::LocalTrackEvent(track_id, event) => match event {
                LocalTrackOutgoingEvent::MediaPacket(pkt) => {
                    if track_id == LOCAL_TRACK_AUDIO_MAIN {
                        self.rtp_engine.send(pkt);
                    }
                }
                _ => {}
            },
            TransportOutgoingEvent::Rpc(rpc) => match rpc {
                EndpointRpcOut::ConnectionAcceptRequest => {
                    self.accept(now_ms).log_error("Should accept call");
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    fn on_custom_event(&mut self, _now_ms: u64, _event: ()) -> Result<(), TransportError> {
        Ok(())
    }

    async fn recv(&mut self, now_ms: u64) -> Result<TransportIncomingEvent<RmIn, RrIn, RlIn>, TransportError> {
        while let Some(action) = self.logic.pop_action() {
            match action {
                ProcessorAction::SendResponse(addr, res) => {
                    self.socket.send_to(addr, SipMessage::Response(res)).map_err(|_e| TransportError::NetworkError)?;
                }
                ProcessorAction::SendRequest(addr, req) => {
                    self.socket.send_to(addr, SipMessage::Request(req)).map_err(|_e| TransportError::NetworkError)?;
                }
                ProcessorAction::Finished(_res) => {
                    //TODO handle error or not
                    self.actions.push_back(TransportIncomingEvent::State(TransportStateEvent::Disconnected));
                    self.socket.close().await;
                }
                _ => {}
            }
        }

        if let Some(event) = self.actions.pop_front() {
            return Ok(event);
        }

        select! {
            event = self.socket.recv().fuse() => {
                let msg = event.map_err(|_e| TransportError::NetworkError)?;
                match msg {
                    SipMessage::Request(req) => {
                        self.logic.on_req(now_ms, req).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
                    }
                    SipMessage::Response(res) => {
                        self.logic.on_res(now_ms, res).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
                    }
                }
                Ok(TransportIncomingEvent::Continue)
            }
            event = self.rtp_engine.recv().fuse() => {
                let rtp = event.ok_or_else(|| TransportError::NetworkError)?;
                let event = TransportIncomingEvent::RemoteTrackEvent(REMOTE_TRACK_AUDIO_MAIN, RemoteTrackIncomingEvent::MediaPacket(rtp));
                self.actions.push_back(event);
                Ok(TransportIncomingEvent::Continue)
            }
        }
    }

    async fn close(&mut self, now_ms: u64) {
        self.logic.close(now_ms);
    }
}
