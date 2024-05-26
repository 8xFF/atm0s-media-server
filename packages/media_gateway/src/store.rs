use media_server_protocol::protobuf::cluster_gateway::ping_event::{gateway_origin::Location, Origin, ServiceStats};

use self::service::ServiceStore;

mod service;

pub struct PingEvent {
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub origin: Origin,
    pub webrtc: Option<ServiceStats>,
}

pub struct GatewayStore {
    location: Location,
    webrtc: ServiceStore,
    output: Option<PingEvent>,
}

impl GatewayStore {
    pub fn new(location: Location) -> Self {
        Self {
            webrtc: ServiceStore::new(location.clone()),
            location,
            output: None,
        }
    }

    pub fn on_tick(&mut self, now: u64) {}

    pub fn on_ping(&mut self, now: u64, from: u32, ping: PingEvent) {
        let webrtc_usage = webrtc_usage(&ping, 80);
        match ping.origin {
            Origin::Media(_) => {
                if let Some(usage) = webrtc_usage {
                    self.webrtc.on_node_ping(from, usage);
                } else {
                    self.webrtc.remove_node(from);
                }
            }
            Origin::Gateway(gateway) => {
                if let Some(usage) = webrtc_usage {
                    self.webrtc.on_gateway_ping(gateway.zone, from, gateway.location.unwrap_or_default(), usage);
                } else {
                    self.webrtc.remove_gateway(gateway.zone, from);
                }
            }
        }
    }

    pub fn best_for(&self, location: Location) -> Option<u32> {
        None
    }

    pub fn pop_output(&mut self) -> Option<PingEvent> {
        self.output.take()
    }
}

fn webrtc_usage(ping: &PingEvent, max_memory: u8) -> Option<u8> {
    if ping.memory as u8 >= max_memory {
        return None;
    }
    let webrtc = ping.webrtc.as_ref()?;
    webrtc.active.then(|| (ping.cpu as u8).max(((webrtc.live * 100) / webrtc.max) as u8))
}
