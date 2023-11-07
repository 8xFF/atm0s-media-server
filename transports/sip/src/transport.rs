use std::collections::VecDeque;

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use futures::{select, FutureExt};
use rsip::{
    typed::{Contact, ContentType, MediaType},
    Host, HostWithPort, Uri,
};
use transport::{Transport, TransportError, TransportIncomingEvent, TransportOutgoingEvent, TransportStateEvent};

use crate::{
    processor::{call_in::CallInProcessor, Processor, ProcessorAction},
    rtp_engine::RtpEngine,
    sip_request::SipRequest,
    virtual_socket::VirtualSocket,
    GroupId, SipMessage,
};

type RmIn = EndpointRpcIn;
type RrIn = RemoteTrackRpcIn;
type RlIn = LocalTrackRpcIn;
type RmOut = EndpointRpcOut;
type RrOut = RemoteTrackRpcOut;
type RlOut = LocalTrackRpcOut;

pub struct SipTransport {
    started_ms: u64,
    need_answer: bool,
    need_ring: bool,
    rtp_engine: RtpEngine,
    socket: VirtualSocket<GroupId, SipMessage>,
    logic: CallInProcessor,
    actions: VecDeque<TransportIncomingEvent<RmIn, RrIn, RlIn>>,
}

impl SipTransport {
    pub async fn new(now_ms: u64, socket: VirtualSocket<GroupId, SipMessage>, req: SipRequest) -> Self {
        let local_contact = Contact {
            uri: Uri {
                scheme: Some(rsip::Scheme::Sip),
                host_with_port: HostWithPort {
                    host: Host::IpAddr("192.168.66.113".parse().expect("")),
                    port: None,
                },
                ..Default::default()
            },
            display_name: None,
            params: vec![],
        };
        let mut rtp_engine = RtpEngine::new().await;
        //TODO dont use unwrap
        log::info!("create transport {}", req.body_str());
        rtp_engine.process_remote_sdp(&req.body_str()).await.unwrap();
        Self {
            started_ms: now_ms,
            need_answer: true,
            need_ring: true,
            rtp_engine,
            socket,
            logic: CallInProcessor::new(now_ms, local_contact, req),
            actions: VecDeque::new(),
        }
    }
}

#[async_trait::async_trait]
impl Transport<(), RmIn, RrIn, RlIn, RmOut, RrOut, RlOut> for SipTransport {
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        self.logic.on_tick(now_ms);

        if self.need_ring && self.started_ms + 1000 < now_ms {
            log::info!("Will ring now");
            self.need_ring = false;
            self.logic.ringing(now_ms);
        }

        if self.need_answer && self.started_ms + 10000 < now_ms {
            self.need_answer = false;
            let local_sdp = self.rtp_engine.create_local_sdp();
            log::info!("Will accept now {}", local_sdp);
            self.logic.accept(now_ms, Some((ContentType(MediaType::Sdp(vec![])).into(), local_sdp.as_bytes().to_vec())));
        }

        Ok(())
    }

    fn on_event(&mut self, _now_ms: u64, event: TransportOutgoingEvent<RmOut, RrOut, RlOut>) -> Result<(), TransportError> {
        Ok(())
    }

    fn on_custom_event(&mut self, _now_ms: u64, _event: ()) -> Result<(), TransportError> {
        //TODO handle reject
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
                ProcessorAction::Finished(res) => {
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
                        self.logic.on_req(now_ms, req);
                    }
                    SipMessage::Response(res) => {
                        self.logic.on_res(now_ms, res);
                    }
                }
            }
            event = self.rtp_engine.recv().fuse() => {
                let rtp = event.ok_or_else(|| TransportError::NetworkError)?;
                self.rtp_engine.send(rtp).await;
            }
        };

        Ok(TransportIncomingEvent::Continue)
    }

    async fn close(&mut self) {
        self.socket.close().await;
    }
}
