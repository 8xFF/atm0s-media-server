use std::collections::VecDeque;

use cluster::rpc::connector::{MediaEndpointEvent, MediaEndpointLogRequest};
use media_utils::F32;
use transport::{TransportError, TransportIncomingEvent, TransportStateEvent};

use crate::{MediaEndpointMiddleware, MediaEndpointMiddlewareOutput};

pub struct MediaEndpointEventLogger {
    started_ms: Option<u64>,
    outputs: VecDeque<crate::MediaEndpointMiddlewareOutput>,
}

impl MediaEndpointEventLogger {
    pub fn new() -> Self {
        Self {
            outputs: VecDeque::new(),
            started_ms: None,
        }
    }

    fn build_event(&self, now_ms: u64, event: MediaEndpointEvent) -> MediaEndpointMiddlewareOutput {
        log::info!("sending event out to connector {:?}", event);
        let event = MediaEndpointLogRequest::SessionEvent {
            ip: "127.0.0.1".to_string(), //TODO
            version: None,
            location: None,
            token: vec![],
            ts: now_ms,
            session_uuid: 0, //TODO
            event,
        };
        MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::MediaEndpointLog(event))
    }
}

impl MediaEndpointMiddleware for MediaEndpointEventLogger {
    fn on_start(&mut self, now_ms: u64) {
        self.started_ms = Some(now_ms);
        self.outputs.push_back(self.build_event(
            now_ms,
            MediaEndpointEvent::Connecting {
                user_agent: "TODO".to_string(), //TODO
                remote: None,                   //TODO
            },
        ));
    }

    fn on_tick(&mut self, _now_ms: u64) {}

    /// return true if event is consumed
    fn on_transport(&mut self, now_ms: u64, event: &TransportIncomingEvent<crate::EndpointRpcIn, crate::rpc::RemoteTrackRpcIn, crate::rpc::LocalTrackRpcIn>) -> bool {
        match event {
            TransportIncomingEvent::State(state) => match state {
                TransportStateEvent::Connected => {
                    self.outputs.push_back(self.build_event(
                        now_ms,
                        MediaEndpointEvent::Connected {
                            after_ms: (now_ms - self.started_ms.expect("Should has started")) as u32,
                            remote: None, //TODO
                        },
                    ));
                }
                TransportStateEvent::Reconnecting => {
                    self.outputs.push_back(self.build_event(
                        now_ms,
                        MediaEndpointEvent::Reconnecting {
                            reason: "TODO".to_string(), //TODO
                        },
                    ));
                }
                TransportStateEvent::Reconnected => {
                    self.outputs.push_back(self.build_event(
                        now_ms,
                        MediaEndpointEvent::Reconnected {
                            remote: None, //TODO
                        },
                    ));
                }
                TransportStateEvent::Disconnected => {
                    self.outputs.push_back(self.build_event(
                        now_ms,
                        MediaEndpointEvent::Disconnected {
                            error: None,
                            duration_ms: now_ms - self.started_ms.expect("Should has started"),
                            received_bytes: 0,  //TODO
                            rtt: F32::new(0.0), //TODO
                            sent_bytes: 0,      //TODO
                        },
                    ));
                }
            },
            _ => {}
        }
        false
    }

    /// return true if event is consumed
    fn on_transport_error(&mut self, now_ms: u64, error: &TransportError) -> bool {
        match error {
            TransportError::ConnectError(_) => {
                self.outputs.push_back(self.build_event(
                    now_ms,
                    MediaEndpointEvent::ConnectError {
                        remote: None,                      //TODO
                        error_code: "TODO".to_string(),    //TODO
                        error_message: "TODO".to_string(), //TODO
                    },
                ));
            }
            TransportError::ConnectionError(_) => {
                self.outputs.push_back(self.build_event(
                    now_ms,
                    MediaEndpointEvent::Disconnected {
                        error: Some("TIMEOUT".to_string()), //TODO
                        duration_ms: now_ms - self.started_ms.expect("Should has started"),
                        received_bytes: 0,  //TODO
                        rtt: F32::new(0.0), //TODO
                        sent_bytes: 0,      //TODO
                    },
                ));
            }
            _ => {}
        }

        false
    }

    /// return true if event is consumed
    fn on_cluster(&mut self, _now_ms: u64, _event: &cluster::ClusterEndpointIncomingEvent) -> bool {
        false
    }

    fn pop_action(&mut self) -> Option<crate::MediaEndpointMiddlewareOutput> {
        self.outputs.pop_back()
    }
}
