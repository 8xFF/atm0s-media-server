use std::collections::HashMap;

use clap::ValueEnum;
use cluster::{
    implement::NodeId,
    rpc::gateway::{NodePing, NodePong, ServiceInfo},
};
use media_utils::F32;

mod global_registry;
mod inner_registry;

trait ServiceRegistry {
    fn on_tick(&mut self, now_ms: u64);
    fn on_ping(&mut self, now_ms: u64, group: &str, location: Option<(F32<2>, F32<2>)>, node_id: NodeId, usage: u8, live: u32, max: u32);
    fn best_nodes(&mut self, location: Option<(F32<2>, F32<2>)>, max_usage: u8, max_usage_fallback: u8, size: usize) -> Vec<NodeId>;
    fn stats(&self) -> ServiceInfo;
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum GatewayMode {
    Global,
    Inner,
}

/// Represents the type of service.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ServiceType {
    Webrtc,
    Rtmp,
    Sip,
}

pub struct GatewayStats {
    pub rtmp: Option<ServiceInfo>,
    pub sip: Option<ServiceInfo>,
    pub webrtc: Option<ServiceInfo>,
}

/// Represents the gateway logic for handling node pings and managing services.
pub struct GatewayLogic {
    mode: GatewayMode,
    services: HashMap<ServiceType, Box<dyn ServiceRegistry>>,
}

impl GatewayLogic {
    /// Creates a new instance of `GatewayLogic`.
    pub fn new(mode: GatewayMode) -> Self {
        Self { mode, services: Default::default() }
    }

    /// Handles the tick event.
    pub fn on_tick(&mut self, now_ms: u64) {
        for (_typ, service) in self.services.iter_mut() {
            service.on_tick(now_ms);
        }
    }

    /// Handles the ping event for a node.
    ///
    /// # Arguments
    ///
    /// * `now_ms` - The current timestamp in milliseconds.
    /// * `ping` - A reference to a `NodePing` struct containing information about the ping.
    ///
    /// # Returns
    ///
    /// A `NodePong` struct with a success flag indicating the success of the ping operation.
    pub fn on_ping(&mut self, now_ms: u64, ping: &NodePing) -> NodePong {
        if let Some(meta) = &ping.webrtc {
            self.on_node_ping_service(now_ms, ping.node_id, ServiceType::Webrtc, &ping.group, ping.location, meta.usage, meta.live, meta.max);
        }
        if let Some(meta) = &ping.rtmp {
            self.on_node_ping_service(now_ms, ping.node_id, ServiceType::Rtmp, &ping.group, ping.location, meta.usage, meta.live, meta.max);
        }
        if let Some(meta) = &ping.sip {
            self.on_node_ping_service(now_ms, ping.node_id, ServiceType::Sip, &ping.group, ping.location, meta.usage, meta.live, meta.max);
        }
        NodePong { success: true }
    }

    /// Returns the best nodes for a service.
    ///
    /// # Arguments
    ///
    /// * `service` - The type of service.
    /// * `max_usage` - The maximum usage value.
    /// * `max_usage_fallback` - The maximum usage fallback value.
    /// * `size` - The size of the result vector.
    ///
    /// # Returns
    ///
    /// A vector of `NodeId` representing the best nodes for the service.
    pub fn best_nodes(&mut self, location: Option<(F32<2>, F32<2>)>, service: ServiceType, max_usage: u8, max_usage_fallback: u8, size: usize) -> Vec<NodeId> {
        self.services
            .get_mut(&service)
            .map(|s| s.best_nodes(location, max_usage, max_usage_fallback, size))
            .unwrap_or_else(|| vec![])
    }

    /// Handles the ping event for a specific service of a node.
    ///
    /// # Arguments
    ///
    /// * `now_ms` - The current timestamp in milliseconds.
    /// * `node_id` - The ID of the node.
    /// * `service` - The type of service.
    /// * `usage` - The usage value.
    /// * `max` - The maximum value.
    fn on_node_ping_service(&mut self, now_ms: u64, node_id: NodeId, service: ServiceType, group: &str, location: Option<(F32<2>, F32<2>)>, usage: u8, live: u32, max: u32) {
        let service = self.services.entry(service).or_insert_with(|| match self.mode {
            GatewayMode::Global => Box::new(global_registry::ServiceGlobalRegistry::new(service)),
            GatewayMode::Inner => Box::new(inner_registry::ServiceInnerRegistry::new(service)),
        });
        service.on_ping(now_ms, group, location, node_id, usage, live, max);
    }

    /// Returns the statistics for the gateway server.
    ///
    /// # Returns
    ///
    /// A `GatewayStats` struct containing the statistics for each service.
    pub fn stats(&self) -> GatewayStats {
        let rtmp = None;
        let sip = None;
        let mut webrtc = None;

        for (service, registry) in &self.services {
            match service {
                ServiceType::Webrtc => webrtc = Some(registry.stats()),
                // ServiceType::Rtmp => rtmp = Some(registry.stats()), //TODO support rtmp
                // ServiceType::Sip => sip = Some(registry.stats()), //TODO support sip
                _ => {}
            }
        }

        GatewayStats { rtmp, sip, webrtc }
    }
}

#[cfg(test)]
mod tests {
    use cluster::rpc::gateway::{NodePing, ServiceInfo};

    use crate::server::gateway::logic::{GatewayLogic, GatewayMode};

    #[test]
    fn test_gateway_logic_creation() {
        let gateway_logic = GatewayLogic::new(GatewayMode::Inner);
        assert_eq!(gateway_logic.services.len(), 0);
    }

    #[test]
    fn test_on_tick_without_services() {
        let mut gateway_logic = GatewayLogic::new(GatewayMode::Inner);
        gateway_logic.on_tick(0);
    }

    #[test]
    fn test_on_ping_with_valid_node_ping() {
        let mut gateway_logic = GatewayLogic::new(GatewayMode::Inner);
        let node_ping = NodePing {
            node_id: 1,
            group: "".to_string(),
            location: None,
            webrtc: Some(ServiceInfo {
                usage: 50,
                live: 50,
                max: 100,
                addr: None,
                domain: None,
            }),
            rtmp: Some(ServiceInfo {
                usage: 30,
                live: 24,
                max: 80,
                addr: Some("127.0.0.1:1935".parse().expect("")),
                domain: None,
            }),
            sip: None,
        };
        let node_pong = gateway_logic.on_ping(0, &node_ping);
        assert_eq!(node_pong.success, true);

        assert_eq!(gateway_logic.services.len(), 2);
    }

    #[test]
    fn test_on_ping_with_no_services() {
        let mut gateway_logic = GatewayLogic::new(GatewayMode::Inner);
        let node_ping = NodePing {
            node_id: 1,
            group: "".to_string(),
            location: None,
            webrtc: None,
            rtmp: None,
            sip: None,
        };
        let node_pong = gateway_logic.on_ping(0, &node_ping);
        assert_eq!(node_pong.success, true);
    }
}
