use media_server_protocol::protobuf::cluster_gateway::ping_event::{gateway_origin::Location, ServiceStats};

use crate::ServiceKind;

const PING_TIMEOUT: u64 = 5000; //timeout after 5s not ping

/// This is for node inside same zone
struct NodeSource {
    node: u32,
    usage: u8,
    stats: ServiceStats,
    last_updated: u64,
}

/// This is for other cluster
struct ZoneSource {
    zone: u32,
    usage: u8,
    location: Location,
    last_updated: u64,
    gateways: Vec<NodeSource>,
}

pub struct ServiceStore {
    kind: ServiceKind,
    location: Location,
    local_sources: Vec<NodeSource>,
    zone_sources: Vec<ZoneSource>,
}

impl ServiceStore {
    pub fn new(kind: ServiceKind, location: Location) -> Self {
        log::info!("[ServiceStore {:?}] create new in {:?}", kind, location);
        Self {
            kind,
            location,
            local_sources: vec![],
            zone_sources: vec![],
        }
    }

    pub fn on_tick(&mut self, now: u64) {
        self.local_sources.retain(|s| s.last_updated + PING_TIMEOUT > now);
        for z in self.zone_sources.iter_mut() {
            z.gateways.retain(|s| s.last_updated + PING_TIMEOUT > now);
        }
        self.zone_sources.retain(|s| !s.gateways.is_empty());
    }

    pub fn on_node_ping(&mut self, now: u64, node: u32, usage: u8, stats: ServiceStats) {
        if let Some(s) = self.local_sources.iter_mut().find(|s| s.node == node) {
            s.usage = usage;
            s.last_updated = now;
        } else {
            log::info!("[ServiceStore {:?}] new node {} usage {}, stats {:?}", self.kind, node, usage, stats);
            self.local_sources.push(NodeSource {
                node,
                usage,
                last_updated: now,
                stats,
            });
        }
        self.local_sources.sort();
    }

    pub fn remove_node(&mut self, node: u32) {
        if let Some((index, _)) = self.local_sources.iter_mut().enumerate().find(|(_i, s)| s.node == node) {
            let node = self.local_sources.remove(index);
            log::info!("[ServiceStore {:?}] remove node {} usage {}, stats {:?}", self.kind, node.node, node.usage, node.stats);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_gateway_ping(&mut self, now: u64, zone: u32, gateway: u32, gateway_usage: u8, location: Location, usage: u8, stats: ServiceStats) {
        if let Some(z) = self.zone_sources.iter_mut().find(|s| s.zone == zone) {
            z.usage = usage;
            z.last_updated = now;
            if let Some(g) = z.gateways.iter_mut().find(|g| g.node == gateway) {
                g.usage = gateway_usage;
                g.last_updated = now;
            } else {
                log::info!(
                    "[ServiceStore {:?}] zone {zone} at {:?} add new gateway {gateway} gateway usage {gateway_usage}, stats {:?}",
                    self.kind,
                    z.location,
                    stats
                );
                z.gateways.push(NodeSource {
                    node: gateway,
                    usage: gateway_usage,
                    last_updated: now,
                    stats,
                });
            }
            z.gateways.sort();
        } else {
            log::info!(
                "[ServiceStore {:?}] new zone {zone} at {:?} usage {usage}, gateway {gateway} gateway usage {gateway_usage}, stats {:?}",
                self.kind,
                location,
                stats
            );
            self.zone_sources.push(ZoneSource {
                zone,
                usage,
                location,
                last_updated: now,
                gateways: vec![NodeSource {
                    node: gateway,
                    usage: gateway_usage,
                    last_updated: now,
                    stats,
                }],
            });
        }
        self.local_sources.sort();
    }

    pub fn remove_gateway(&mut self, zone: u32, gateway: u32) {
        if let Some((index, z)) = self.zone_sources.iter_mut().enumerate().find(|(_i, z)| z.zone == zone) {
            if let Some((g_index, _g)) = z.gateways.iter_mut().enumerate().find(|(_i, g)| g.node == gateway) {
                let g = z.gateways.remove(g_index);
                log::info!(
                    "[ServiceStore {:?}] zone {zone} at {:?} remove gateway {} gateway usage {}, stats {:?}",
                    self.kind,
                    z.location,
                    g.node,
                    g.usage,
                    g.stats,
                );
            }
            if z.gateways.is_empty() {
                let zone = self.zone_sources.remove(index);
                log::info!("[ServiceStore {:?}] remove zone {} at {:?}", self.kind, zone.zone, zone.location,);
            }
        }
    }

    pub fn best_for(&self, location: Option<Location>) -> Option<u32> {
        let location = location.unwrap_or_else(|| self.location.clone());
        let mut min_dis = distance(&self.location, &location);
        let mut min_node = self.local_sources.first().map(|s| s.node);

        for z in self.zone_sources.iter() {
            let dis = distance(&location, &z.location);
            if min_node.is_none() || min_dis > dis {
                min_dis = dis;
                min_node = z.gateways.first().map(|s| s.node);
            }
        }

        log::info!("[ServiceStore {:?}] query best node for {:?} got min_dis {min_dis} min_node {:?}", self.kind, location, min_node);
        min_node
    }

    pub fn local_stats(&self) -> Option<ServiceStats> {
        if self.local_sources.is_empty() {
            return None;
        }

        let mut stats = ServiceStats { active: false, max: 0, live: 0 };
        for n in self.local_sources.iter() {
            if n.stats.active {
                stats.active = true;
            }
            stats.live += n.stats.live;
            stats.max += n.stats.max;
        }

        Some(stats)
    }
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
        Some(self.usage.cmp(&other.usage))
    }
}

impl Eq for ZoneSource {}
impl PartialEq for ZoneSource {
    fn eq(&self, other: &Self) -> bool {
        self.zone.eq(&other.zone)
    }
}

impl Ord for ZoneSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.usage.cmp(&other.usage)
    }
}

impl PartialOrd for ZoneSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.usage.cmp(&other.usage))
    }
}

/// Calculate distance between two nodes.
fn distance(node1: &Location, node2: &Location) -> f32 {
    //TODO make it more accuracy
    ((node1.lat - node2.lat).powi(2) + (node1.lon - node2.lon).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use media_server_protocol::protobuf::cluster_gateway::ping_event::{gateway_origin::Location, ServiceStats};

    use crate::{store::service::PING_TIMEOUT, ServiceKind};

    use super::ServiceStore;

    #[test]
    fn empty_store() {
        let store = ServiceStore::new(ServiceKind::Webrtc, Location { lat: 1.0, lon: 1.0 });
        assert_eq!(store.best_for(None), None);
        assert_eq!(store.best_for(Some(Location { lat: 1.0, lon: 1.0 })), None);

        assert_eq!(store.local_stats(), None);
    }

    #[test]
    fn local_store() {
        let mut store = ServiceStore::new(ServiceKind::Webrtc, Location { lat: 1.0, lon: 1.0 });

        store.on_node_ping(0, 1, 60, ServiceStats { live: 100, max: 1000, active: true });
        store.on_node_ping(0, 2, 50, ServiceStats { live: 60, max: 1000, active: true });

        //should got lowest usage
        assert_eq!(store.best_for(None), Some(2));
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(2));
        assert_eq!(store.local_stats(), Some(ServiceStats { live: 160, max: 2000, active: true }));

        //after node2 increase usage should fallback to node1
        store.on_node_ping(0, 2, 61, ServiceStats { live: 120, max: 1000, active: true });

        assert_eq!(store.best_for(None), Some(1));
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(1));

        //after remove should fallback to remain
        store.remove_node(1);

        assert_eq!(store.best_for(None), Some(2));
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(2));
    }

    #[test]
    fn remote_zones_store() {
        let mut store = ServiceStore::new(ServiceKind::Webrtc, Location { lat: 1.0, lon: 1.0 });

        store.on_gateway_ping(0, 256, 256, 60, Location { lat: 2.0, lon: 2.0 }, 50, ServiceStats { live: 100, max: 1000, active: true });
        store.on_gateway_ping(0, 256, 257, 50, Location { lat: 2.0, lon: 2.0 }, 50, ServiceStats { live: 100, max: 1000, active: true });

        //should got lowest usage gateway node
        assert_eq!(store.best_for(None), Some(257));
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(257));

        //after gateway 257 increase usage should switch to 256
        store.on_gateway_ping(0, 256, 257, 65, Location { lat: 2.0, lon: 2.0 }, 50, ServiceStats { live: 100, max: 1000, active: true });

        assert_eq!(store.best_for(None), Some(256));
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(256));

        //should fallback to remain gateway
        store.remove_gateway(256, 256);

        assert_eq!(store.best_for(None), Some(257));
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(257));
    }

    #[test]
    fn local_and_remote_zones() {
        let mut store = ServiceStore::new(ServiceKind::Webrtc, Location { lat: 1.0, lon: 1.0 });

        store.on_node_ping(0, 1, 60, ServiceStats { live: 100, max: 1000, active: true });
        store.on_gateway_ping(0, 256, 257, 60, Location { lat: 2.0, lon: 2.0 }, 50, ServiceStats { live: 100, max: 1000, active: true });

        //should got local zone if don't provide location
        assert_eq!(store.best_for(None), Some(1));

        //should got closest zone gaetway
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(257));

        //after remove local should fallback to other zone
        store.remove_node(1);

        assert_eq!(store.best_for(None), Some(257));
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), Some(257));

        //after remove other zone should return None
        store.remove_gateway(256, 257);

        assert_eq!(store.best_for(None), None);
        assert_eq!(store.best_for(Some(Location { lat: 2.0, lon: 2.0 })), None);
    }

    #[test]
    fn clear_timeout() {
        let mut store = ServiceStore::new(ServiceKind::Webrtc, Location { lat: 1.0, lon: 1.0 });

        store.on_node_ping(0, 1, 60, ServiceStats { live: 100, max: 1000, active: true });
        store.on_gateway_ping(0, 256, 257, 60, Location { lat: 2.0, lon: 2.0 }, 50, ServiceStats { live: 100, max: 1000, active: true });

        assert_eq!(store.local_sources.len(), 1);
        assert_eq!(store.zone_sources.len(), 1);

        store.on_tick(PING_TIMEOUT);

        assert_eq!(store.local_sources.len(), 0);
        assert_eq!(store.zone_sources.len(), 0);
    }
}
