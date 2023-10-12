use std::collections::VecDeque;

use cluster::{ClusterRoomIncomingEvent, ClusterRoomOutgoingEvent};
use transport::{MediaIncomingEvent, MediaOutgoingEvent};

pub enum MediaInternalAction {
    Endpoint(MediaOutgoingEvent),
    Cluster(ClusterRoomOutgoingEvent),
}

pub struct MediaEndpointInteral {
    output_actions: VecDeque<MediaInternalAction>,
}

impl MediaEndpointInteral {
    pub fn new() -> Self {
        Self { output_actions: VecDeque::new() }
    }

    pub fn on_tick(&mut self, now_ms: u64) {}

    pub fn on_transport(&mut self, event: MediaIncomingEvent) {
        match event {
            MediaIncomingEvent::Connected => {}
            MediaIncomingEvent::Reconnecting => {}
            MediaIncomingEvent::Reconnected => {}
            MediaIncomingEvent::Disconnected => {}
            MediaIncomingEvent::Continue => {}
            MediaIncomingEvent::Media(_, _) => {}
            MediaIncomingEvent::Data(data) => {}
            MediaIncomingEvent::Stats { rtt, loss, jitter, bitrate } => todo!(),
        }
    }

    pub fn on_cluster(&mut self, event: ClusterRoomIncomingEvent) {}

    pub fn pop_action(&mut self) -> Option<MediaInternalAction> {
        self.output_actions.pop_front()
    }
}
