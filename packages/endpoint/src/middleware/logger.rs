use std::collections::VecDeque;

use protocol::media_event_logs::{
    session_event::{self, SessionConnectError, SessionConnected, SessionConnecting, SessionDisconnected, SessionReconnected, SessionReconnecting},
    F32p2, MediaEndpointLogEvent, MediaEndpointLogRequest, SessionEvent,
};
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

    fn build_event(&self, now_ms: u64, event: session_event::Event) -> MediaEndpointMiddlewareOutput {
        log::info!("sending event out to connector {:?}", event);
        let event: MediaEndpointLogRequest = MediaEndpointLogRequest {
            event: Some(MediaEndpointLogEvent::SessionEvent(SessionEvent {
                ip: "127.0.0.1".to_string(), //TODO
                version: None,
                location: None,
                token: vec![],
                ts: now_ms,
                session_uuid: 0, //TODO
                event: Some(event),
            })),
        };

        MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::MediaEndpointLog(event))
    }
}

impl MediaEndpointMiddleware for MediaEndpointEventLogger {
    fn on_start(&mut self, now_ms: u64) {
        self.started_ms = Some(now_ms);
        self.outputs.push_back(self.build_event(
            now_ms,
            session_event::Event::Connecting(SessionConnecting {
                user_agent: "TODO".to_string(), //TODO
                remote: None,                   //TODO
            }),
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
                        session_event::Event::Connected(SessionConnected {
                            after_ms: (now_ms - self.started_ms.expect("Should has started")) as u32,
                            remote: None, //TODO
                        }),
                    ));
                }
                TransportStateEvent::Reconnecting => {
                    self.outputs.push_back(self.build_event(
                        now_ms,
                        session_event::Event::Reconnecting(SessionReconnecting {
                            reason: "TODO".to_string(), //TODO
                        }),
                    ));
                }
                TransportStateEvent::Reconnected => {
                    self.outputs.push_back(self.build_event(
                        now_ms,
                        session_event::Event::Reconnected(SessionReconnected{
                            remote: None, //TODO
                        }),
                    ));
                }
                TransportStateEvent::Disconnected => {
                    self.outputs.push_back(self.build_event(
                        now_ms,
                        session_event::Event::Disconnected(SessionDisconnected {
                            error: None,
                            duration_ms: now_ms - self.started_ms.expect("Should has started"),
                            received_bytes: 0,             //TODO
                            rtt: Some(F32p2 { value: 0 }), //TODO
                            sent_bytes: 0,                 //TODO
                        }),
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
                    session_event::Event::ConnectError(SessionConnectError {
                        remote: None,                      //TODO
                        error_code: "TODO".to_string(),    //TODO
                        error_message: "TODO".to_string(), //TODO
                    }),
                ));
            }
            TransportError::ConnectionError(_) => {
                self.outputs.push_back(self.build_event(
                    now_ms,
                    session_event::Event::Disconnected(SessionDisconnected {
                        error: Some("TIMEOUT".to_string()), //TODO
                        duration_ms: now_ms - self.started_ms.expect("Should has started"),
                        received_bytes: 0,             //TODO
                        rtt: Some(F32p2 { value: 0 }), //TODO
                        sent_bytes: 0,
                    }),
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

    fn pop_action(&mut self, _now_ms: u64) -> Option<crate::MediaEndpointMiddlewareOutput> {
        self.outputs.pop_back()
    }

    fn before_drop(&mut self, _now_ms: u64) {}
}
#[cfg(test)]
mod tests {
    use super::*;
    use protocol::media_event_logs::session_event::*;

    #[test]
    fn test_on_transport_connected() {
        let mut logger = MediaEndpointEventLogger::new();
        let event = TransportIncomingEvent::State(TransportStateEvent::Connected);
        logger.on_start(0);
        logger.on_transport(1000, &event);
        assert_eq!(
            logger.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::MediaEndpointLog(
                MediaEndpointLogRequest {
                    event: Some(MediaEndpointLogEvent::SessionEvent(SessionEvent {
                        ip: "127.0.0.1".to_string(),
                        version: None,
                        location: None,
                        token: vec![],
                        ts: 1000,
                        session_uuid: 0,
                        event: Some(Event::Connected(SessionConnected { after_ms: 1000, remote: None })),
                    })),
                }
            )))
        );
    }

    #[test]
    fn test_on_transport_reconnecting() {
        let mut logger = MediaEndpointEventLogger::new();
        let event = TransportIncomingEvent::State(TransportStateEvent::Reconnecting);
        logger.on_start(0);
        logger.on_transport(1000, &event);
        assert_eq!(
            logger.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::MediaEndpointLog(
                MediaEndpointLogRequest {
                    event: Some(MediaEndpointLogEvent::SessionEvent(SessionEvent {
                        ip: "127.0.0.1".to_string(),
                        version: None,
                        location: None,
                        token: vec![],
                        ts: 1000,
                        session_uuid: 0,
                        event: Some(Event::Reconnecting(SessionReconnecting { reason: "TODO".to_string() })),
                    })),
                }
            )))
        );
    }

    #[test]
    fn test_on_transport_reconnected() {
        let mut logger = MediaEndpointEventLogger::new();
        let event = TransportIncomingEvent::State(TransportStateEvent::Reconnected);
        logger.on_start(0);
        logger.on_transport(1000, &event);
        assert_eq!(
            logger.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::MediaEndpointLog(
                MediaEndpointLogRequest {
                    event: Some(MediaEndpointLogEvent::SessionEvent(SessionEvent {
                        ip: "127.0.0.1".to_string(),
                        version: None,
                        location: None,
                        token: vec![],
                        ts: 1000,
                        session_uuid: 0,
                        event: Some(Event::Reconnected(SessionReconnected { remote: None })),
                    })),
                }
            )))
        );
    }

    #[test]
    fn test_on_transport_disconnected() {
        let mut logger = MediaEndpointEventLogger::new();
        let event = TransportIncomingEvent::State(TransportStateEvent::Disconnected);
        logger.on_start(0);
        logger.on_transport(1000, &event);
        assert_eq!(
            logger.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::MediaEndpointLog(
                MediaEndpointLogRequest {
                    event: Some(MediaEndpointLogEvent::SessionEvent(SessionEvent {
                        ip: "127.0.0.1".to_string(),
                        version: None,
                        location: None,
                        token: vec![],
                        ts: 1000,
                        session_uuid: 0,
                        event: Some(Event::Disconnected(SessionDisconnected {
                            error: None,
                            duration_ms: 1000,
                            received_bytes: 0,
                            rtt: Some(F32p2 { value: 0 }),
                            sent_bytes: 0,
                        })),
                    })),
                }
            )))
        );
    }

    #[test]
    fn test_on_transport_error_connect_error() {
        let mut logger = MediaEndpointEventLogger::new();
        let error = TransportError::ConnectError(transport::ConnectErrorReason::Timeout);
        logger.on_start(0);
        logger.on_transport_error(1000, &error);
        assert_eq!(
            logger.pop_action(0),
            Some(MediaEndpointMiddlewareOutput::Cluster(cluster::ClusterEndpointOutgoingEvent::MediaEndpointLog(
                MediaEndpointLogRequest {
                    event: Some(MediaEndpointLogEvent::SessionEvent(SessionEvent {
                        ip: "127.0.0.1".to_string(),
                        version: None,
                        location: None,
                        token: vec![],
                        ts: 1000,
                        session_uuid: 0,
                        event: Some(Event::ConnectError(SessionConnectError {
                            remote: None,
                            error_code: "TODO".to_string(),
                            error_message: "TODO".to_string(),
                        })),
                    })),
                }
            )))
        );
    }
}
