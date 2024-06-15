use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use atm0s_sdn::NodeId;
use media_server_protocol::cluster::{ClusterGatewayInfo, ClusterMediaInfo, ClusterNodeGenericInfo, ClusterNodeInfo};

const NODE_TIMEOUT: u64 = 30_000;

#[derive(poem_openapi::Object)]
pub struct ConsoleNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
}

#[derive(poem_openapi::Object)]
pub struct GatewayNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub live: u32,
    pub max: u32,
}

#[derive(poem_openapi::Object)]
pub struct MediaNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub live: u32,
    pub max: u32,
}

#[derive(poem_openapi::Object)]
pub struct ConnectorNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
}

#[derive(poem_openapi::Object)]
pub struct Zone {
    pub lat: f32,
    pub lon: f32,
    pub zone_id: u32,
    pub consoles: usize,
    pub gateways: usize,
    pub medias: usize,
    pub connectors: usize,
}

#[derive(poem_openapi::Object)]
pub struct ZoneDetails {
    pub lat: f32,
    pub lon: f32,
    pub consoles: Vec<ConsoleNode>,
    pub gateways: Vec<GatewayNode>,
    pub medias: Vec<MediaNode>,
    pub connectors: Vec<ConnectorNode>,
}

#[derive(Debug)]
struct ConsoleContainer {
    last_updated: u64,
    generic: ClusterNodeGenericInfo,
}

#[derive(Debug)]
struct GatewayContainer {
    last_updated: u64,
    generic: ClusterNodeGenericInfo,
    info: ClusterGatewayInfo,
}

#[derive(Debug)]
struct MediaContainer {
    last_updated: u64,
    generic: ClusterNodeGenericInfo,
    info: ClusterMediaInfo,
}

#[derive(Debug, Default)]
struct ZoneContainer {
    lat: f32,
    lon: f32,
    consoles: HashMap<u32, ConsoleContainer>,
    gateways: HashMap<u32, GatewayContainer>,
    medias: HashMap<u32, MediaContainer>,
}

#[derive(Debug, Default)]
struct Storage {
    zones: HashMap<u32, ZoneContainer>,
}

impl Storage {
    pub fn on_tick(&mut self, now: u64) {
        for (_, zone) in self.zones.iter_mut() {
            zone.consoles.retain(|_, g| g.last_updated + NODE_TIMEOUT > now);
            zone.gateways.retain(|_, g| g.last_updated + NODE_TIMEOUT > now);
            zone.medias.retain(|_, g| g.last_updated + NODE_TIMEOUT > now);
        }
        self.zones.retain(|_, z| z.consoles.len() + z.gateways.len() + z.medias.len() > 0);
    }

    pub fn on_ping(&mut self, now: u64, node: NodeId, info: ClusterNodeInfo) {
        match info {
            ClusterNodeInfo::Console(generic) => {
                let zone_id = node & 0xFF_FF_FF_00;
                log::info!("Zone {zone_id} on console ping, zones {}", self.zones.len());
                let zone = self.zones.entry(zone_id).or_insert_with(Default::default);
                zone.consoles.insert(node, ConsoleContainer { last_updated: now, generic });
                log::info!("Zone {zone_id} on console ping, after zones {}", self.zones.len());
            }
            ClusterNodeInfo::Gateway(generic, info) => {
                let zone_id = node & 0xFF_FF_FF_00;
                log::info!("Zone {zone_id} on gateway ping");
                let zone = self.zones.entry(zone_id).or_insert_with(Default::default);
                zone.lat = info.lat;
                zone.lon = info.lon;
                zone.gateways.insert(node, GatewayContainer { last_updated: now, generic, info });
            }
            ClusterNodeInfo::Media(generic, info) => {
                let zone_id = node & 0xFF_FF_FF_00;
                log::info!("Zone {zone_id} on media ping");
                let zone = self.zones.entry(zone_id).or_insert_with(Default::default);
                zone.medias.insert(node, MediaContainer { last_updated: now, generic, info });
            }
        }
    }

    pub fn zones(&self) -> Vec<Zone> {
        self.zones
            .iter()
            .map(|(id, z)| Zone {
                lat: z.lat,
                lon: z.lon,
                zone_id: *id,
                consoles: z.consoles.len(),
                gateways: z.gateways.len(),
                medias: z.medias.len(),
                connectors: 0,
            })
            .collect::<Vec<_>>()
    }

    pub fn zone(&self, zone_id: u32) -> Option<ZoneDetails> {
        let z = self.zones.get(&zone_id)?;
        Some(ZoneDetails {
            lat: z.lat,
            lon: z.lon,
            consoles: z
                .consoles
                .iter()
                .map(|(id, g)| ConsoleNode {
                    addr: g.generic.addr.clone(),
                    node_id: *id,
                    cpu: g.generic.cpu,
                    memory: g.generic.memory,
                    disk: g.generic.disk,
                })
                .collect::<Vec<_>>(),
            gateways: z
                .gateways
                .iter()
                .map(|(id, g)| GatewayNode {
                    addr: g.generic.addr.clone(),
                    node_id: *id,
                    cpu: g.generic.cpu,
                    memory: g.generic.memory,
                    disk: g.generic.disk,
                    live: g.info.live,
                    max: g.info.max,
                })
                .collect::<Vec<_>>(),
            medias: z
                .medias
                .iter()
                .map(|(id, g)| MediaNode {
                    addr: g.generic.addr.clone(),
                    node_id: *id,
                    cpu: g.generic.cpu,
                    memory: g.generic.memory,
                    disk: g.generic.disk,
                    live: g.info.live,
                    max: g.info.max,
                })
                .collect::<Vec<_>>(),
            connectors: vec![],
        })
    }
}

#[derive(Default, Clone)]
pub struct StorageShared {
    storage: Arc<RwLock<Storage>>,
}

impl StorageShared {
    pub fn on_tick(&self, now: u64) {
        self.storage.write().expect("should lock storage").on_tick(now);
    }

    pub fn on_ping(&self, now: u64, node: NodeId, info: ClusterNodeInfo) {
        self.storage.write().expect("should lock storage").on_ping(now, node, info);
    }

    pub fn zones(&self) -> Vec<Zone> {
        self.storage.read().expect("should lock storage").zones()
    }

    pub fn zone(&self, zone_id: u32) -> Option<ZoneDetails> {
        self.storage.read().expect("should lock storage").zone(zone_id)
    }
}
