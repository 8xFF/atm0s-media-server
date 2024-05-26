use std::collections::HashMap;

use media_server_protocol::protobuf::cluster_gateway::ping_event::{gateway_origin::Location, Origin, ServiceStats};

const PING_TIMEOUT: u64 = 5000; //timeout after 5s not ping

/// This is for node inside same zone
struct NodeSource {
    node: u32,
    usage: u8,
    last_updated: u64,
}

/// This is for other cluser
struct GatewaySource {
    zone: u32,
    usage: u8,
    location: Location,
    last_updated: u64,
    gateways: HashMap<u32, u8>,
}

pub struct ServiceStore {
    location: Location,
    local_sources: Vec<NodeSource>,
    remote_sources: Vec<GatewaySource>,
}

impl ServiceStore {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            local_sources: vec![],
            remote_sources: vec![],
        }
    }

    pub fn on_tick(&mut self, now: u64) {}

    pub fn on_node_ping(&mut self, node: u32, usage: u8) {}

    pub fn remove_node(&mut self, node: u32) {}

    pub fn on_gateway_ping(&mut self, zone: u32, gateway: u32, location: Location, usage: u8) {}

    pub fn remove_gateway(&mut self, zone: u32, gateway: u32) {}
}

impl Eq for NodeSource {}
impl PartialEq for NodeSource {
    fn eq(&self, other: &Self) -> bool {
        self.node.eq(&other.node)
    }
}

impl Ord for NodeSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.usage.cmp(&other.usage)
    }
}

impl PartialOrd for NodeSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.usage.partial_cmp(&other.usage)
    }
}

impl Eq for GatewaySource {}
impl PartialEq for GatewaySource {
    fn eq(&self, other: &Self) -> bool {
        self.zone.eq(&other.zone)
    }
}

impl Ord for GatewaySource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.usage.cmp(&other.usage)
    }
}

impl PartialOrd for GatewaySource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.usage.partial_cmp(&other.usage)
    }
}
