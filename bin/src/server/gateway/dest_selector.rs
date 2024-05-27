use std::collections::HashMap;

use media_server_gateway::ServiceKind;
use media_server_protocol::protobuf::cluster_gateway::ping_event::gateway_origin::Location;
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    oneshot,
};

#[derive(Clone)]
pub struct GatewayDestSelector {
    tx: Sender<(ServiceKind, f32, f32, oneshot::Sender<Option<u32>>)>,
}

impl GatewayDestSelector {
    pub async fn select(&self, kind: ServiceKind, lat: f32, lon: f32) -> Option<u32> {
        let (tx, rx) = oneshot::channel();
        self.tx.send((kind, lat, lon, tx)).await.ok()?;
        rx.await.ok()?
    }
}

pub struct GatewayDestRequester {
    rx: Receiver<(ServiceKind, f32, f32, oneshot::Sender<Option<u32>>)>,
    req_seed: u64,
    reqs: HashMap<u64, oneshot::Sender<Option<u32>>>,
}

impl GatewayDestRequester {
    pub fn on_event(&mut self, event: media_server_gateway::store_service::Event) {
        match event {
            media_server_gateway::store_service::Event::FindNodeRes(req_id, res) => {
                if let Some(tx) = self.reqs.remove(&req_id) {
                    if let Err(_) = tx.send(res) {
                        log::error!("[GatewayDestRequester] answer for req_id {req_id} error");
                    }
                }
            }
        }
    }

    pub fn recv(&mut self) -> Option<media_server_gateway::store_service::Control> {
        let (kind, lat, lon, tx) = self.rx.try_recv().ok()?;
        let req_id = self.req_seed;
        self.req_seed += 1;
        self.reqs.insert(req_id, tx);
        Some(media_server_gateway::store_service::Control::FindNodeReq(req_id, kind, Location { lat, lon }))
    }
}

pub fn build_dest_selector() -> (GatewayDestSelector, GatewayDestRequester) {
    let (tx, rx) = channel(100);
    (
        GatewayDestSelector { tx },
        GatewayDestRequester {
            rx,
            req_seed: 0,
            reqs: HashMap::new(),
        },
    )
}
