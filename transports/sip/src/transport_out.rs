use std::{collections::VecDeque, net::SocketAddr};

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{select, FutureExt};
use rsip::{
    headers::CallId,
    typed::{Contact, From, To},
    Host, HostWithPort, Uri,
};
use transport::{Transport, TransportError, TransportIncomingEvent, TransportOutgoingEvent, TransportRuntimeError, TransportStateEvent};

use crate::{
    processor::{
        call_out::{CallOutProcessor, CallOutProcessorAction},
        Processor, ProcessorAction,
    },
    rtp_engine::{RtpEngine, RtpEngineError},
    virtual_socket::VirtualSocket,
    GroupId, SipMessage,
};

type RmIn = EndpointRpcIn;
type RrIn = RemoteTrackRpcIn;
type RlIn = LocalTrackRpcIn;
type RmOut = EndpointRpcOut;
type RrOut = RemoteTrackRpcOut;
type RlOut = LocalTrackRpcOut;

pub struct SipTransportOut {
    rtp_engine: RtpEngine,
    socket: VirtualSocket<GroupId, SipMessage>,
    logic: CallOutProcessor,
    actions: VecDeque<TransportIncomingEvent<RmIn, RrIn, RlIn>>,
}

impl SipTransportOut {
    pub async fn new(now_ms: u64, bind_addr: SocketAddr, call_id: CallId, local_from: From, remote_to: To, socket: VirtualSocket<GroupId, SipMessage>) -> Result<Self, RtpEngineError> {
        let local_contact = Contact {
            uri: Uri {
                scheme: Some(rsip::Scheme::Sip),
                host_with_port: HostWithPort {
                    host: Host::IpAddr(bind_addr.ip()),
                    port: Some(bind_addr.port().into()),
                },
                ..Default::default()
            },
            display_name: None,
            params: vec![],
        };
        let mut rtp_engine = RtpEngine::new(bind_addr.ip()).await;

        Ok(Self {
            logic: CallOutProcessor::new(now_ms, local_contact, call_id, local_from, remote_to, &rtp_engine.create_local_sdp()),
            rtp_engine,
            socket,
            actions: VecDeque::new(),
        })
    }

    pub fn cancel(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.logic.cancel(now_ms).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        Ok(())
    }

    pub fn end(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.logic.end(now_ms).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Transport<(), RmIn, RrIn, RlIn, RmOut, RrOut, RlOut> for SipTransportOut {
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.logic.on_tick(now_ms).map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
        Ok(())
    }

    fn on_event(&mut self, _now_ms: u64, _event: TransportOutgoingEvent<RmOut, RrOut, RlOut>) -> Result<(), TransportError> {
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
                ProcessorAction::LogicOutput(out) => match out {
                    CallOutProcessorAction::Accepted(body) => {
                        if let Some((_typ, body)) = body {
                            if let Ok(sdp) = String::from_utf8(body) {
                                self.rtp_engine
                                    .process_remote_sdp(&sdp)
                                    .await
                                    .map_err(|_| TransportError::RuntimeError(TransportRuntimeError::ProtocolError))?;
                            }
                        }
                    }
                },
            }
        }

        if let Some(event) = self.actions.pop_front() {
            return Ok(event);
        }

        let rtp_out = select! {
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
                None
            }
            event = self.rtp_engine.recv().fuse() => {
                let rtp = event.ok_or_else(|| TransportError::NetworkError)?;
                //TODO send to cluster insteand of echoback
                Some(rtp)
            }
        };

        //TODO don't echoback
        if let Some(rtp) = rtp_out {
            self.rtp_engine.send(rtp).await;
        }

        Ok(TransportIncomingEvent::Continue)
    }

    async fn close(&mut self) {
        self.socket.close().await;
    }
}
