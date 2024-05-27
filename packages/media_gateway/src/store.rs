use media_server_protocol::protobuf::cluster_gateway::ping_event::{gateway_origin::Location, GatewayOrigin, Origin, ServiceStats};

use crate::ServiceKind;

use self::service::ServiceStore;

mod service;

#[derive(Debug)]
pub struct PingEvent {
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub origin: Origin,
    pub webrtc: Option<ServiceStats>,
}

pub struct GatewayStore {
    zone: u32,
    location: Location,
    webrtc: ServiceStore,
    output: Option<PingEvent>,
}

impl GatewayStore {
    pub fn new(zone: u32, location: Location) -> Self {
        Self {
            webrtc: ServiceStore::new(ServiceKind::Webrtc, location.clone()),
            zone,
            location,
            output: None,
        }
    }

    pub fn on_tick(&mut self, now: u64) {
        self.webrtc.on_tick(now);

        let ping = PingEvent {
            cpu: 0,    //TODO
            memory: 0, //TODO
            disk: 0,   //TODO
            origin: Origin::Gateway(GatewayOrigin {
                zone: self.zone,
                location: Some(self.location.clone()),
            }),
            webrtc: self.webrtc.local_stats(),
        };

        log::trace!("[GatewayStore] create ping event for broadcast {:?}", ping);
        self.output = Some(ping);
    }

    pub fn on_ping(&mut self, now: u64, from: u32, ping: PingEvent) {
        log::debug!("[GatewayStore] on ping from {from} data {:?}", ping);
        let node_usage = node_usage(&ping, 80, 90);
        let webrtc_usage = webrtc_usage(&ping, 80, 90);
        match ping.origin {
            Origin::Media(_) => match (node_usage, webrtc_usage, ping.webrtc) {
                (Some(_node), Some(webrtc), Some(stats)) => self.webrtc.on_node_ping(now, from, webrtc, stats),
                _ => self.webrtc.remove_node(from),
            },
            Origin::Gateway(gateway) => {
                if gateway.zone == self.zone {
                    //Reject stats from same zone
                    return;
                }
                match (node_usage, webrtc_usage, gateway.location, ping.webrtc) {
                    (Some(node), Some(webrtc), Some(location), Some(stats)) => self.webrtc.on_gateway_ping(now, gateway.zone, from, node, location, webrtc, stats),
                    _ => self.webrtc.remove_gateway(gateway.zone, from),
                }
            }
        }
    }

    pub fn best_for(&self, kind: ServiceKind, location: Location) -> Option<u32> {
        let node = match kind {
            ServiceKind::Webrtc => self.webrtc.best_for(&location),
        };
        log::debug!("[GatewayStore] query best {:?} for {:?} got {:?}", kind, location, node);
        node
    }

    pub fn pop_output(&mut self) -> Option<PingEvent> {
        self.output.take()
    }
}

fn node_usage(ping: &PingEvent, max_memory: u8, max_disk: u8) -> Option<u8> {
    if ping.memory as u8 >= max_memory {
        return None;
    }

    if ping.disk as u8 >= max_disk {
        return None;
    }

    Some(ping.cpu)
}

fn webrtc_usage(ping: &PingEvent, max_memory: u8, max_disk: u8) -> Option<u8> {
    if ping.memory as u8 >= max_memory {
        return None;
    }

    if ping.disk as u8 >= max_disk {
        return None;
    }

    let webrtc = ping.webrtc.as_ref()?;
    webrtc.active.then(|| (ping.cpu as u8).max(((webrtc.live * 100) / webrtc.max) as u8))
}
