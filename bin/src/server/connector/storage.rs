use media_server_protocol::protobuf::cluster_connector::{connector_request, peer_event};

pub struct ConnectorStorage {}

impl ConnectorStorage {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn on_event(&mut self, _req_id: u64, event: connector_request::Event) -> Option<()> {
        match event {
            connector_request::Event::Peer(event) => self.on_peer_event(event.session_id, event.event?).await,
        }
    }

    async fn on_peer_event(&mut self, session: u64, event: peer_event::Event) -> Option<()> {
        match event {
            peer_event::Event::RouteBegin(_) => todo!(),
            peer_event::Event::RouteSuccess(_) => todo!(),
            peer_event::Event::RouteError(_) => todo!(),
            peer_event::Event::Connecting(_) => todo!(),
            peer_event::Event::Connected(_) => todo!(),
            peer_event::Event::ConnectError(_) => todo!(),
            peer_event::Event::Stats(_) => todo!(),
            peer_event::Event::Reconnect(_) => todo!(),
            peer_event::Event::Reconnected(_) => todo!(),
            peer_event::Event::Disconnected(_) => todo!(),
        }
    }
}
