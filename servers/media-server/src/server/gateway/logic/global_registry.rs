use std::collections::HashMap;

use cluster::{implement::NodeId, rpc::gateway::ServiceInfo, GATEWAY_SERVICE};
use media_utils::F32;
use metrics::{describe_gauge, gauge};

use super::{RouteResult, ServiceRegistry, ServiceType};

const NODE_TIMEOUT_MS: u64 = 10_000;

fn lat_lng_distance(from: &(F32<2>, F32<2>), to: &(F32<2>, F32<2>)) -> f32 {
    let (from_lat, from_lng) = from;
    let (to_lat, to_lng) = to;
    let r = 6371.0; // radius of the earth in kilometers

    let dlat = (from_lat.value() - to_lat.value()).to_radians();
    let dlon = (from_lng.value() - to_lng.value()).to_radians();

    let a = (dlat / 2.0).sin() * (dlat / 2.0).sin() + from_lat.value().to_radians().cos() * to_lat.value().to_radians().cos() * (dlon / 2.0).sin() * (dlon / 2.0).sin();
    let c = 2.0 * ((a.sqrt()).atan2((1.0 - a).sqrt()));

    r * c
}

#[derive(Debug, Default)]
struct Zone {
    zone: String,
    location: (F32<2>, F32<2>),
    nodes: HashMap<NodeId, u64>,
    usage: u8,
    live: u32,
    max: u32,
    last_updated: u64,
}

#[derive(Debug)]
pub(super) struct ServiceGlobalRegistry {
    zone: String,
    metric_live: String,
    metric_max: String,
    zones: HashMap<String, Zone>,
}

impl ServiceGlobalRegistry {
    pub fn new(zone: &str, service: ServiceType) -> Self {
        let metric_live = format!("gateway.sessions.{:?}.live", service);
        let metric_max = format!("gateway.sessions.{:?}.max", service);
        describe_gauge!(metric_live.clone(), format!("Current live {:?} sessions number", service));
        describe_gauge!(metric_max.clone(), format!("Max live {:?} sessions number", service));

        Self {
            zone: zone.to_string(),
            metric_live,
            metric_max,
            zones: Default::default(),
        }
    }

    fn sum_live(&self) -> u32 {
        self.zones.iter().map(|(_, s)| s.live).sum()
    }

    fn sum_max(&self) -> u32 {
        self.zones.iter().map(|(_, s)| s.max).sum()
    }

    fn closest_zone(&self, location: &(F32<2>, F32<2>), max_usage: u8) -> Option<&Zone> {
        let mut closest_zone = None;
        let mut closest_distance = std::f32::MAX;
        for (_, zone) in self.zones.iter() {
            let distance = lat_lng_distance(location, &zone.location);
            if distance < closest_distance && zone.max > 0 && zone.usage <= max_usage {
                closest_distance = distance;
                closest_zone = Some(zone);
            }
        }
        closest_zone
    }
}

impl ServiceRegistry for ServiceGlobalRegistry {
    /// remove not that dont received ping in NODE_TIMEOUT_MS
    fn on_tick(&mut self, now_ms: u64) {
        self.zones.retain(|_, s| s.last_updated + NODE_TIMEOUT_MS > now_ms);
        for (_, zone) in self.zones.iter_mut() {
            zone.nodes.retain(|_, &mut last_updated| last_updated + NODE_TIMEOUT_MS > now_ms);
        }
        gauge!(self.metric_live.clone()).set(self.sum_live() as f64);
        gauge!(self.metric_max.clone()).set(self.sum_max() as f64);
    }

    /// we save node or create new, then sort by ascending order
    fn on_ping(&mut self, now_ms: u64, zone: &str, location: Option<(F32<2>, F32<2>)>, node_id: NodeId, usage: u8, live: u32, max: u32) {
        let location = location.unwrap_or((F32::<2>::new(0.0), F32::<2>::new(0.0)));

        if let Some(slot) = self.zones.get_mut(zone) {
            slot.nodes.insert(node_id, now_ms);
            slot.usage = usage;
            slot.live = live;
            slot.max = max;
            slot.last_updated = now_ms;
        } else {
            self.zones.insert(
                zone.to_string(),
                Zone {
                    zone: zone.to_string(),
                    nodes: HashMap::from([(node_id, now_ms)]),
                    location,
                    usage,
                    live,
                    max,
                    last_updated: now_ms,
                },
            );
        }
    }

    /// we get first with max_usage, if not enough => using max_usage_fallback
    fn best_nodes(&mut self, location: Option<(F32<2>, F32<2>)>, max_usage: u8, max_usage_fallback: u8, size: usize) -> RouteResult {
        let location = location.unwrap_or((F32::<2>::new(0.0), F32::<2>::new(0.0)));

        //finding closest zone
        let mut closest_zone = self.closest_zone(&location, max_usage);
        if closest_zone.is_none() {
            closest_zone = self.closest_zone(&location, max_usage_fallback);
        }

        if let Some(zone) = closest_zone {
            if zone.zone.eq(&self.zone) {
                RouteResult::LocalNode
            } else {
                let mut nodes = zone.nodes.keys().cloned().collect::<Vec<_>>();
                nodes.truncate(size);
                RouteResult::OtherNode { nodes, service_id: GATEWAY_SERVICE }
            }
        } else {
            RouteResult::NotFound
        }
    }

    fn stats(&self) -> ServiceInfo {
        panic!("dont support")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::gateway::logic::ServiceType;

    // ServiceGlobalRegistry can be created with default values
    #[test]
    fn test_service_registry_creation() {
        let mut registry = ServiceGlobalRegistry::new("local", ServiceType::Webrtc);
        assert_eq!(registry.zones.len(), 0);
        assert_eq!(registry.best_nodes(None, 80, 90, 1), RouteResult::NotFound);
    }

    // test with single zone and single gateway
    #[test]
    fn test_service_registry_single_zone_single_gateway() {
        let mut registry = ServiceGlobalRegistry::new("zone1", ServiceType::Webrtc);
        let now_ms = 0;
        let zone = "zone1";
        let location = Some((F32::<2>::new(0.0), F32::<2>::new(0.0)));
        let node_id = 1;
        let usage = 0;
        let live = 0;
        let max = 10;

        registry.on_ping(now_ms, zone, location, node_id, usage, live, max);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.len(), 1);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.get(&node_id).unwrap(), &now_ms);
        assert_eq!(registry.zones.get(zone).unwrap().usage, usage);
        assert_eq!(registry.zones.get(zone).unwrap().live, live);
        assert_eq!(registry.zones.get(zone).unwrap().max, max);
        assert_eq!(registry.zones.get(zone).unwrap().last_updated, now_ms);

        assert_eq!(registry.best_nodes(location, 60, 80, 1), RouteResult::LocalNode);

        registry.on_tick(now_ms + NODE_TIMEOUT_MS);
        assert_eq!(registry.zones.len(), 0);
    }

    #[test]
    fn test_service_fallback_max_usage() {
        let mut registry = ServiceGlobalRegistry::new("zone1", ServiceType::Webrtc);
        let now_ms = 0;
        let zone = "zone1";
        let location = Some((F32::<2>::new(0.0), F32::<2>::new(0.0)));
        let node_id = 1;
        let usage = 70;
        let live = 9;
        let max = 10;

        registry.on_ping(now_ms, zone, location, node_id, usage, live, max);
        assert_eq!(registry.best_nodes(location, 50, 60, 1), RouteResult::NotFound);
        assert_eq!(registry.best_nodes(location, 60, 80, 1), RouteResult::LocalNode);
    }

    // test with gateway with max zero should return none
    #[test]
    fn test_service_registry_single_zone_single_gateway_with_max_zero() {
        let mut registry = ServiceGlobalRegistry::new("zone1", ServiceType::Webrtc);
        let now_ms = 0;
        let zone = "zone1";
        let location = Some((F32::<2>::new(0.0), F32::<2>::new(0.0)));
        let node_id = 1;
        let usage = 0;
        let live = 0;
        let max = 0;

        registry.on_ping(now_ms, zone, location, node_id, usage, live, max);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.len(), 1);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.get(&node_id).unwrap(), &now_ms);
        assert_eq!(registry.zones.get(zone).unwrap().usage, usage);
        assert_eq!(registry.zones.get(zone).unwrap().live, live);
        assert_eq!(registry.zones.get(zone).unwrap().max, max);
        assert_eq!(registry.zones.get(zone).unwrap().last_updated, now_ms);

        assert_eq!(registry.best_nodes(location, 60, 80, 1), RouteResult::NotFound);

        registry.on_tick(now_ms + NODE_TIMEOUT_MS);
        assert_eq!(registry.zones.len(), 0);
    }

    // test with single zone multi gateways
    #[test]
    fn test_service_registry_single_zone_multi_gateways() {
        let mut registry = ServiceGlobalRegistry::new("zone1", ServiceType::Webrtc);
        let now_ms = 0;
        let zone = "zone1";
        let location = Some((F32::<2>::new(0.0), F32::<2>::new(0.0)));
        let node_id_1 = 1;
        let node_id_2 = 2;
        let usage = 0;
        let live = 0;
        let max = 10;

        registry.on_ping(now_ms, zone, location, node_id_1, usage, live, max);
        registry.on_ping(now_ms, zone, location, node_id_2, usage, live, max);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.len(), 2);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.get(&node_id_1).unwrap(), &now_ms);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.get(&node_id_2).unwrap(), &now_ms);
        assert_eq!(registry.zones.get(zone).unwrap().usage, usage);
        assert_eq!(registry.zones.get(zone).unwrap().live, live);
        assert_eq!(registry.zones.get(zone).unwrap().max, max);
        assert_eq!(registry.zones.get(zone).unwrap().last_updated, now_ms);

        let route_res = registry.best_nodes(location, 60, 80, 2);
        assert_eq!(route_res, RouteResult::LocalNode);

        registry.on_ping(1000, zone, location, node_id_1, usage, live, max);

        //simulate timeout
        registry.on_tick(now_ms + NODE_TIMEOUT_MS);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.len(), 1);
        assert_eq!(registry.zones.get(zone).unwrap().nodes.get(&node_id_1).unwrap(), &1000);
        assert_eq!(registry.zones.get(zone).unwrap().usage, usage);
        assert_eq!(registry.zones.get(zone).unwrap().live, live);
        assert_eq!(registry.zones.get(zone).unwrap().max, max);
        assert_eq!(registry.zones.get(zone).unwrap().last_updated, 1000);
    }

    //test with multi zones and multi gateways
    #[test]
    fn test_service_registry_multi_zones_multi_gateways() {
        let mut registry = ServiceGlobalRegistry::new("zone1", ServiceType::Webrtc);
        let now_ms = 0;
        let zone_1 = "zone1";
        let zone_2 = "zone2";
        let location_1 = Some((F32::<2>::new(0.0), F32::<2>::new(0.0)));
        let location_2 = Some((F32::<2>::new(1.0), F32::<2>::new(1.0)));
        let node_id_1 = 1;
        let node_id_2 = 2;
        let usage = 0;
        let live = 0;
        let max = 10;

        registry.on_ping(now_ms, zone_1, location_1, node_id_1, usage, live, max);
        registry.on_ping(now_ms, zone_2, location_2, node_id_2, usage, live, max);

        assert_eq!(registry.zones.len(), 2);
        assert_eq!(registry.zones.get(zone_1).unwrap().nodes.len(), 1);
        assert_eq!(registry.zones.get(zone_1).unwrap().nodes.get(&node_id_1).unwrap(), &now_ms);
        assert_eq!(registry.zones.get(zone_1).unwrap().usage, usage);
        assert_eq!(registry.zones.get(zone_1).unwrap().live, live);
        assert_eq!(registry.zones.get(zone_1).unwrap().max, max);
        assert_eq!(registry.zones.get(zone_1).unwrap().last_updated, now_ms);

        assert_eq!(registry.zones.get(zone_2).unwrap().nodes.len(), 1);
        assert_eq!(registry.zones.get(zone_2).unwrap().nodes.get(&node_id_2).unwrap(), &now_ms);
        assert_eq!(registry.zones.get(zone_2).unwrap().usage, usage);
        assert_eq!(registry.zones.get(zone_2).unwrap().live, live);
        assert_eq!(registry.zones.get(zone_2).unwrap().max, max);
        assert_eq!(registry.zones.get(zone_2).unwrap().last_updated, now_ms);

        let route_res = registry.best_nodes(location_1, 60, 80, 2);
        assert_eq!(route_res, RouteResult::LocalNode);

        let route_res = registry.best_nodes(location_2, 60, 80, 2);
        assert_eq!(
            route_res,
            RouteResult::OtherNode {
                nodes: vec![node_id_2],
                service_id: GATEWAY_SERVICE
            }
        );
    }

    #[test]
    fn test_distance_function() {
        let from = (F32::<2>::new(0.0), F32::<2>::new(0.0));
        let to = (F32::<2>::new(1.0), F32::<2>::new(1.0));
        assert_eq!(F32::<2>::new(lat_lng_distance(&from, &to)), F32::<2>::new(157.24939));
    }
}
