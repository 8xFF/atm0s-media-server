use std::cmp::Ordering;

use cluster::{implement::NodeId, rpc::gateway::ServiceInfo, MEDIA_SERVER_SERVICE};
use media_utils::F32;
use metrics::{describe_gauge, gauge};

use super::{RouteResult, ServiceRegistry, ServiceType};

const NODE_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Default)]
struct NodeSlot {
    node_id: NodeId,
    usage: u8,
    live: u32,
    max: u32,
    last_updated: u64,
}

impl PartialEq for NodeSlot {
    fn eq(&self, other: &Self) -> bool {
        self.usage.eq(&other.usage) && self.max.eq(&other.max)
    }
}

impl PartialOrd for NodeSlot {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.usage.partial_cmp(&other.usage) {
            Some(Ordering::Equal) => Some(self.max.partial_cmp(&other.max)?.reverse()),
            r => r,
        }
    }
}

impl Eq for NodeSlot {}

impl Ord for NodeSlot {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).expect("Should compare")
    }
}

#[derive(Debug)]
pub(super) struct ServiceInnerRegistry {
    metric_live: String,
    metric_max: String,
    nodes: Vec<NodeSlot>,
}

impl ServiceInnerRegistry {
    pub fn new(service: ServiceType) -> Self {
        let metric_live = format!("gateway.sessions.{:?}.live", service);
        let metric_max = format!("gateway.sessions.{:?}.max", service);
        describe_gauge!(metric_live.clone(), format!("Current live {:?} sessions number", service));
        describe_gauge!(metric_max.clone(), format!("Max live {:?} sessions number", service));

        Self {
            metric_live,
            metric_max,
            nodes: Default::default(),
        }
    }

    fn sum_live(&self) -> u32 {
        self.nodes.iter().map(|s| s.live).sum()
    }

    fn sum_max(&self) -> u32 {
        self.nodes.iter().map(|s| s.max).sum()
    }
}

impl ServiceRegistry for ServiceInnerRegistry {
    /// remove not that dont received ping in NODE_TIMEOUT_MS
    fn on_tick(&mut self, now_ms: u64) {
        self.nodes.retain(|s| s.last_updated + NODE_TIMEOUT_MS > now_ms);
        gauge!(self.metric_live.clone()).set(self.sum_live() as f64);
        gauge!(self.metric_max.clone()).set(self.sum_max() as f64);
    }

    /// we save node or create new, then sort by ascending order
    fn on_ping(&mut self, now_ms: u64, _zone: &str, _location: Option<(F32<2>, F32<2>)>, node_id: NodeId, usage: u8, live: u32, max: u32) {
        if let Some(slot) = self.nodes.iter_mut().find(|s| s.node_id == node_id) {
            slot.usage = usage;
            slot.live = live;
            slot.max = max;
            slot.last_updated = now_ms;
        } else {
            self.nodes.push(NodeSlot {
                node_id,
                usage,
                live,
                max,
                last_updated: now_ms,
            });
        }
        self.nodes.sort();
    }

    /// we get first with max_usage, if not enough => using max_usage_fallback
    fn best_nodes(&mut self, _location: Option<(F32<2>, F32<2>)>, max_usage: u8, max_usage_fallback: u8, size: usize) -> RouteResult {
        let mut nodes = vec![];
        for slot in self.nodes.iter().rev() {
            if slot.usage <= max_usage {
                nodes.push(slot.node_id);
                if nodes.len() == size {
                    break;
                }
            }
        }

        if nodes.len() < size {
            for slot in self.nodes.iter().rev() {
                if slot.usage <= max_usage_fallback {
                    if !nodes.contains(&slot.node_id) {
                        nodes.push(slot.node_id);
                        if nodes.len() == size {
                            break;
                        }
                    }
                }
            }
        }

        if nodes.is_empty() {
            RouteResult::NotFound
        } else {
            RouteResult::OtherNode {
                nodes,
                service_id: MEDIA_SERVER_SERVICE,
            }
        }
    }

    fn stats(&self) -> ServiceInfo {
        let sum_max = self.sum_max();
        let usage = if sum_max == 0 {
            0
        } else {
            (self.sum_live() as f64 / sum_max as f64 * 100.0) as u8
        };

        ServiceInfo {
            usage,
            live: self.sum_live(),
            max: self.sum_max(),
            addr: None,
            domain: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::gateway::logic::ServiceType;

    // ServiceInnerRegistry can be created with default values
    #[test]
    fn test_service_registry_creation() {
        let registry = ServiceInnerRegistry::new(ServiceType::Webrtc);
        assert_eq!(
            registry.stats(),
            ServiceInfo {
                usage: 0,
                live: 0,
                max: 0,
                addr: None,
                domain: None,
            }
        );
        assert_eq!(registry.nodes.len(), 0);
    }

    // on_ping adds a new node to the registry
    #[test]
    fn test_on_ping_adds_new_node() {
        let mut registry = ServiceInnerRegistry::new(ServiceType::Webrtc);
        let now_ms = 0;
        let node_id = 1;
        let live = 50;
        let usage = 50;
        let max = 100;

        registry.on_ping(now_ms, "", None, node_id, usage, live, max);

        assert_eq!(registry.nodes.len(), 1);
        assert_eq!(registry.nodes[0].node_id, node_id);
        assert_eq!(registry.nodes[0].usage, usage);
        assert_eq!(registry.nodes[0].live, live);
        assert_eq!(registry.nodes[0].max, max);
        assert_eq!(registry.nodes[0].last_updated, now_ms);

        assert_eq!(registry.sum_live(), live);
        assert_eq!(registry.sum_max(), max);
    }

    // on_ping updates an existing node in the registry
    #[test]
    fn test_on_ping_updates_existing_node() {
        let mut registry = ServiceInnerRegistry::new(ServiceType::Webrtc);
        let now_ms = 0;
        let node_id = 1;
        let live = 50;
        let usage = 50;
        let max = 100;

        registry.on_ping(now_ms, "", None, node_id, usage, live, max);

        let new_usage = 75;
        let new_live = 112;
        let new_max = 150;

        registry.on_ping(now_ms + 1000, "", None, node_id, new_usage, new_live, new_max);

        assert_eq!(registry.nodes.len(), 1);
        assert_eq!(registry.nodes[0].node_id, node_id);
        assert_eq!(registry.nodes[0].live, new_live);
        assert_eq!(registry.nodes[0].usage, new_usage);
        assert_eq!(registry.nodes[0].max, new_max);
        assert_eq!(registry.nodes[0].last_updated, now_ms + 1000);
    }

    // on_tick removes all nodes when all nodes haven't received a ping in NODE_TIMEOUT_MS
    #[test]
    fn test_on_tick_removes_all_nodes() {
        let mut registry = ServiceInnerRegistry::new(ServiceType::Webrtc);
        let now_ms = 0;
        let node_id1 = 1;
        let live1 = 50;
        let usage1 = 50;
        let max1 = 100;
        registry.on_ping(now_ms, "", None, node_id1, usage1, live1, max1);

        let node_id2 = 2;
        let live2 = 112;
        let usage2 = 75;
        let max2 = 150;
        registry.on_ping(now_ms, "", None, node_id2, usage2, live2, max2);

        registry.on_tick(now_ms + NODE_TIMEOUT_MS + 1);

        assert_eq!(registry.nodes.len(), 0);
    }

    #[test]
    fn test_best_nodes_returns_nodes_with_max_usage() {
        let mut registry = ServiceInnerRegistry::new(ServiceType::Webrtc);
        let now_ms = 0;
        let node_id1 = 1;
        let live1 = 50;
        let usage1 = 50;
        let max1 = 100;
        registry.on_ping(now_ms, "", None, node_id1, usage1, live1, max1);

        let node_id2 = 2;
        let usage2 = 75;
        let live2 = 112;
        let max2 = 150;
        registry.on_ping(now_ms, "", None, node_id2, usage2, live2, max2);

        assert_eq!(registry.sum_live(), live1 + live2);
        assert_eq!(registry.sum_max(), max1 + max2);

        let max_usage = 60;
        let max_usage_fallback = 70;
        let size = 2;

        let result = registry.best_nodes(None, max_usage, max_usage_fallback, size);

        assert_eq!(
            result,
            RouteResult::OtherNode {
                nodes: vec![node_id1],
                service_id: MEDIA_SERVER_SERVICE
            }
        );
    }

    #[test]
    fn test_best_nodes_returns_nodes_with_max_usage_fallback() {
        let mut registry = ServiceInnerRegistry::new(ServiceType::Webrtc);
        let now_ms = 0;
        let node_id1 = 1;
        let usage1 = 50;
        let live1 = 50;
        let max1 = 100;
        registry.on_ping(now_ms, "", None, node_id1, usage1, live1, max1);

        let node_id2 = 2;
        let usage2 = 75;
        let live2 = 112;
        let max2 = 150;
        registry.on_ping(now_ms, "", None, node_id2, usage2, live2, max2);

        let max_usage = 60;
        let max_usage_fallback = 80;
        let size = 2;

        let mut result = registry.best_nodes(None, max_usage_fallback, max_usage, size);
        if let RouteResult::OtherNode { nodes, service_id: _ } = &mut result {
            nodes.sort();
        }

        assert_eq!(
            result,
            RouteResult::OtherNode {
                nodes: vec![node_id1, node_id2],
                service_id: MEDIA_SERVER_SERVICE
            }
        );
    }

    #[test]
    fn test_stats() {
        let mut registry = ServiceInnerRegistry::new(ServiceType::Webrtc);
        let now_ms = 0;
        let node_id1 = 1;
        let usage1 = 50;
        let live1 = 50;
        let max1 = 100;
        registry.on_ping(now_ms, "", None, node_id1, usage1, live1, max1);

        let node_id2 = 2;
        let usage2 = 100;
        let live2 = 100;
        let max2 = 200;
        registry.on_ping(now_ms, "", None, node_id2, usage2, live2, max2);

        let stats = registry.stats();

        assert_eq!(stats.usage, 50);
        assert_eq!(stats.live, 150);
        assert_eq!(stats.max, 300);
        assert_eq!(stats.addr, None);
        assert_eq!(stats.domain, None);
    }
}
