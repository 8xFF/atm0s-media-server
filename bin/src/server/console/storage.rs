use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use atm0s_sdn::{services::visualization::ConnectionInfo, NodeId};
use media_server_protocol::cluster::{ClusterGatewayInfo, ClusterMediaInfo, ClusterNodeGenericInfo, ClusterNodeInfo, ZoneId};
use serde::{Deserialize, Serialize};

const NODE_TIMEOUT: u64 = 30_000;

#[derive(poem_openapi::Object, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub id: u64,
    pub node: NodeId,
    pub addr: String,
    pub rtt_ms: u32,
}

impl From<ConnectionInfo> for Connection {
    fn from(value: ConnectionInfo) -> Self {
        Self {
            id: value.conn.session(),
            node: value.dest,
            addr: value.remote.to_string(),
            rtt_ms: value.rtt_ms,
        }
    }
}

#[derive(poem_openapi::Object, Debug, PartialEq, Eq)]
pub struct ConsoleNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub conns: Vec<Connection>,
}

#[derive(poem_openapi::Object, Debug, PartialEq, Eq)]
pub struct GatewayNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub conns: Vec<Connection>,
    pub disk: u8,
    pub live: u32,
    pub max: u32,
}

#[derive(poem_openapi::Object, Debug, PartialEq, Eq)]
pub struct MediaNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub conns: Vec<Connection>,
    pub live: u32,
    pub max: u32,
}

#[derive(poem_openapi::Object, Debug, PartialEq, Eq)]
pub struct ConnectorNode {
    pub addr: String,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub conns: Vec<Connection>,
}

#[derive(poem_openapi::Object, Debug, PartialEq)]
pub struct Zone {
    pub lat: f32,
    pub lon: f32,
    pub zone_id: u32,
    pub consoles: usize,
    pub gateways: usize,
    pub medias: usize,
    pub connectors: usize,
}

#[derive(poem_openapi::Object, Debug, PartialEq)]
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
    conns: Vec<Connection>,
}

#[derive(Debug)]
struct GatewayContainer {
    last_updated: u64,
    generic: ClusterNodeGenericInfo,
    info: ClusterGatewayInfo,
    conns: Vec<Connection>,
}

#[derive(Debug)]
struct MediaContainer {
    last_updated: u64,
    generic: ClusterNodeGenericInfo,
    info: ClusterMediaInfo,
    conns: Vec<Connection>,
}

#[derive(Debug)]
struct ConnectorContainer {
    last_updated: u64,
    generic: ClusterNodeGenericInfo,
    conns: Vec<Connection>,
}

#[derive(Debug, Default)]
struct ZoneContainer {
    lat: f32,
    lon: f32,
    consoles: HashMap<u32, ConsoleContainer>,
    gateways: HashMap<u32, GatewayContainer>,
    medias: HashMap<u32, MediaContainer>,
    connectors: HashMap<u32, ConnectorContainer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNodeData {
    pub id: NodeId,
    pub zone_id: u32,
    pub info: ClusterNodeGenericInfo,
    pub conns: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum NetworkNodeEvent {
    #[serde(rename = "snapshot")]
    Snapshot(Vec<NetworkNodeData>),
    #[serde(rename = "on_changed")]
    OnChanged(NetworkNodeData),
    #[serde(rename = "on_removed")]
    OnRemoved(Vec<NodeId>),
}

#[derive(Debug)]
struct Storage {
    zones: HashMap<ZoneId, ZoneContainer>,
    sender: tokio::sync::broadcast::Sender<NetworkNodeEvent>,
}

impl Storage {
    pub fn new(sender: tokio::sync::broadcast::Sender<NetworkNodeEvent>) -> Self {
        Self { zones: HashMap::new(), sender }
    }
}

impl Storage {
    pub fn on_tick(&mut self, now: u64) {
        for (_, zone) in self.zones.iter_mut() {
            zone.consoles.retain(|_, g| g.last_updated + NODE_TIMEOUT > now);
            zone.gateways.retain(|_, g| g.last_updated + NODE_TIMEOUT > now);
            zone.medias.retain(|_, g| g.last_updated + NODE_TIMEOUT > now);
            zone.connectors.retain(|_, g| g.last_updated + NODE_TIMEOUT > now);
        }
        self.zones.retain(|_, z| z.consoles.len() + z.gateways.len() + z.medias.len() + z.connectors.len() > 0);
    }

    pub fn on_ping(&mut self, now: u64, node: NodeId, info: ClusterNodeInfo, conns: Vec<ConnectionInfo>) {
        let node_data = NetworkNodeData {
            id: node,
            zone_id: ZoneId::from_node_id(node).0,
            info: match &info {
                ClusterNodeInfo::Console(generic) => generic.clone(),
                ClusterNodeInfo::Gateway(generic, _) => generic.clone(),
                ClusterNodeInfo::Media(generic, _) => generic.clone(),
                ClusterNodeInfo::Connector(generic) => generic.clone(),
            },
            conns: conns.iter().map(|c| c.clone().into()).collect::<Vec<_>>(),
        };

        match info {
            ClusterNodeInfo::Console(generic) => {
                let zone_id = ZoneId::from_node_id(node);
                log::info!("Zone {zone_id:?} on console ping, zones {}", self.zones.len());
                let zone = self.zones.entry(zone_id).or_default();
                zone.consoles.insert(
                    node,
                    ConsoleContainer {
                        last_updated: now,
                        generic,
                        conns: conns.into_iter().map(|c| c.into()).collect::<Vec<_>>(),
                    },
                );
                log::info!("Zone {zone_id:?} on console ping, after zones {}", self.zones.len());
            }
            ClusterNodeInfo::Gateway(generic, info) => {
                let zone_id = ZoneId::from_node_id(node);
                log::info!("Zone {zone_id:?} on gateway ping");
                let zone = self.zones.entry(zone_id).or_default();
                zone.lat = info.lat;
                zone.lon = info.lon;
                zone.gateways.insert(
                    node,
                    GatewayContainer {
                        last_updated: now,
                        generic,
                        info,
                        conns: conns.into_iter().map(|c| c.into()).collect::<Vec<_>>(),
                    },
                );
            }
            ClusterNodeInfo::Media(generic, info) => {
                let zone_id = ZoneId::from_node_id(node);
                log::info!("Zone {zone_id:?} on media ping");
                let zone = self.zones.entry(zone_id).or_default();
                zone.medias.insert(
                    node,
                    MediaContainer {
                        last_updated: now,
                        generic,
                        info,
                        conns: conns.into_iter().map(|c| c.into()).collect::<Vec<_>>(),
                    },
                );
            }
            ClusterNodeInfo::Connector(generic) => {
                let zone_id = ZoneId::from_node_id(node);
                log::info!("Zone {zone_id:?} on connector ping, zones {}", self.zones.len());
                let zone = self.zones.entry(zone_id).or_default();
                zone.connectors.insert(
                    node,
                    ConnectorContainer {
                        last_updated: now,
                        generic,
                        conns: conns.into_iter().map(|c| c.into()).collect::<Vec<_>>(),
                    },
                );
                log::info!("Zone {zone_id:?} on console ping, after zones {}", self.zones.len());
            }
        }

        if let Err(e) = self.sender.send(NetworkNodeEvent::OnChanged(node_data)) {
            log::error!("Failed to send on_changed event: {:?}", e);
        }
    }

    pub fn on_node_removed(&mut self, now: u64, node: NodeId) {
        let zone_id = ZoneId::from_node_id(node);
        let zone = self.zones.entry(zone_id).or_default();
        zone.consoles.remove(&node);
        zone.gateways.remove(&node);
        zone.medias.remove(&node);
        zone.connectors.remove(&node);

        if let Err(e) = self.sender.send(NetworkNodeEvent::OnRemoved(vec![node])) {
            log::error!("Failed to send on_removed event: {:?}", e);
        }
    }

    pub fn consoles(&self) -> Vec<ConsoleNode> {
        self.zones
            .iter()
            .flat_map(|(_id, z)| {
                z.consoles
                    .iter()
                    .map(|(id, g)| ConsoleNode {
                        addr: g.generic.addr.clone(),
                        node_id: *id,
                        cpu: g.generic.cpu,
                        memory: g.generic.memory,
                        disk: g.generic.disk,
                        conns: g.conns.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn zones(&self) -> Vec<Zone> {
        self.zones
            .iter()
            .map(|(id, z)| Zone {
                lat: z.lat,
                lon: z.lon,
                zone_id: id.0,
                consoles: z.consoles.len(),
                gateways: z.gateways.len(),
                medias: z.medias.len(),
                connectors: z.connectors.len(),
            })
            .collect::<Vec<_>>()
    }

    pub fn zone(&self, zone_id: ZoneId) -> Option<ZoneDetails> {
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
                    conns: g.conns.clone(),
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
                    conns: g.conns.clone(),
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
                    conns: g.conns.clone(),
                })
                .collect::<Vec<_>>(),
            connectors: z
                .connectors
                .iter()
                .map(|(id, g)| ConnectorNode {
                    addr: g.generic.addr.clone(),
                    node_id: *id,
                    cpu: g.generic.cpu,
                    memory: g.generic.memory,
                    disk: g.generic.disk,
                    conns: g.conns.clone(),
                })
                .collect::<Vec<_>>(),
        })
    }

    pub fn network_node(&self) -> Vec<NetworkNodeData> {
        self.zones
            .iter()
            .flat_map(|(zone_id, z)| {
                z.consoles
                    .iter()
                    .map(|(id, g)| NetworkNodeData {
                        id: *id,
                        zone_id: zone_id.0,
                        info: g.generic.clone(),
                        conns: g.conns.iter().map(|c| c.clone()).collect::<Vec<_>>(),
                    })
                    .chain(z.gateways.iter().map(|(id, g)| NetworkNodeData {
                        id: *id,
                        zone_id: zone_id.0,
                        info: g.generic.clone(),
                        conns: g.conns.iter().map(|c| c.clone()).collect::<Vec<_>>(),
                    }))
                    .chain(z.medias.iter().map(|(id, g)| NetworkNodeData {
                        id: *id,
                        zone_id: zone_id.0,
                        info: g.generic.clone(),
                        conns: g.conns.iter().map(|c| c.clone()).collect::<Vec<_>>(),
                    }))
                    .chain(z.connectors.iter().map(|(id, g)| NetworkNodeData {
                        id: *id,
                        zone_id: zone_id.0,
                        info: g.generic.clone(),
                        conns: g.conns.iter().map(|c| c.clone()).collect::<Vec<_>>(),
                    }))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct StorageShared {
    storage: Arc<RwLock<Storage>>,
    receiver: Arc<tokio::sync::broadcast::Receiver<NetworkNodeEvent>>,
}

impl Default for StorageShared {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::broadcast::channel(1024);
        Self {
            storage: Arc::new(RwLock::new(Storage::new(sender))),
            receiver: Arc::new(receiver),
        }
    }
}

impl StorageShared {
    pub fn on_tick(&self, now: u64) {
        self.storage.write().expect("should lock storage").on_tick(now);
    }

    pub fn on_ping(&self, now: u64, node: NodeId, info: ClusterNodeInfo, conns: Vec<ConnectionInfo>) {
        self.storage.write().expect("should lock storage").on_ping(now, node, info, conns);
    }

    pub fn on_node_removed(&self, now: u64, node: NodeId) {
        self.storage.write().expect("should lock storage").on_node_removed(now, node);
    }

    pub fn consoles(&self) -> Vec<ConsoleNode> {
        self.storage.read().expect("should lock storage").consoles()
    }

    pub fn zones(&self) -> Vec<Zone> {
        self.storage.read().expect("should lock storage").zones()
    }

    pub fn zone(&self, zone_id: ZoneId) -> Option<ZoneDetails> {
        self.storage.read().expect("should lock storage").zone(zone_id)
    }

    pub fn network_node(&self) -> Vec<NetworkNodeData> {
        self.storage.read().expect("should lock storage").network_node()
    }

    pub fn subcribe(&self) -> tokio::sync::broadcast::Receiver<NetworkNodeEvent> {
        self.receiver.resubscribe()
    }
}

#[cfg(test)]
mod tests {
    use media_server_protocol::cluster::{ClusterGatewayInfo, ClusterMediaInfo, ClusterNodeGenericInfo, ClusterNodeInfo, ZoneId};

    use crate::server::console_storage::{ConnectorNode, ConsoleNode, GatewayNode, MediaNode, Zone, ZoneDetails, NODE_TIMEOUT};

    use super::Storage;

    #[test]
    fn collect_console() {
        let (sender, _receiver) = tokio::sync::broadcast::channel(10);
        let mut storage = Storage::new(sender);

        storage.on_ping(
            0,
            1,
            ClusterNodeInfo::Console(ClusterNodeGenericInfo {
                addr: "addr".to_string(),
                cpu: 11,
                memory: 22,
                disk: 33,
            }),
            vec![],
        );
        storage.on_tick(0);

        assert_eq!(
            storage.zones(),
            vec![Zone {
                lat: 0.0,
                lon: 0.0,
                zone_id: 0,
                consoles: 1,
                gateways: 0,
                medias: 0,
                connectors: 0,
            }]
        );

        assert_eq!(
            storage.zone(ZoneId(0)),
            Some(ZoneDetails {
                lat: 0.0,
                lon: 0.0,
                consoles: vec![ConsoleNode {
                    addr: "addr".to_string(),
                    node_id: 1,
                    cpu: 11,
                    memory: 22,
                    disk: 33,
                    conns: vec![],
                }],
                gateways: vec![],
                medias: vec![],
                connectors: vec![]
            })
        );

        assert_eq!(storage.zone(ZoneId(1)), None);

        storage.on_tick(NODE_TIMEOUT);
        //after timeout should clear
        assert_eq!(storage.zones(), vec![]);
        assert_eq!(storage.zone(ZoneId(0)), None);
    }

    #[test]
    fn collect_gateway() {
        let (sender, _receiver) = tokio::sync::broadcast::channel(10);
        let mut storage = Storage::new(sender);

        storage.on_ping(
            0,
            1,
            ClusterNodeInfo::Gateway(
                ClusterNodeGenericInfo {
                    addr: "addr".to_string(),
                    cpu: 11,
                    memory: 22,
                    disk: 33,
                },
                ClusterGatewayInfo {
                    live: 0,
                    max: 100,
                    lat: 10.0,
                    lon: 11.0,
                },
            ),
            vec![],
        );
        storage.on_tick(0);

        assert_eq!(
            storage.zones(),
            vec![Zone {
                lat: 10.0,
                lon: 11.0,
                zone_id: 0,
                consoles: 0,
                gateways: 1,
                medias: 0,
                connectors: 0,
            }]
        );

        assert_eq!(
            storage.zone(ZoneId(0)),
            Some(ZoneDetails {
                lat: 10.0,
                lon: 11.0,
                consoles: vec![],
                gateways: vec![GatewayNode {
                    addr: "addr".to_string(),
                    node_id: 1,
                    cpu: 11,
                    memory: 22,
                    disk: 33,
                    conns: vec![],
                    live: 0,
                    max: 100,
                }],
                medias: vec![],
                connectors: vec![]
            })
        );

        assert_eq!(storage.zone(ZoneId(1)), None);

        storage.on_tick(NODE_TIMEOUT);
        //after timeout should clear
        assert_eq!(storage.zones(), vec![]);
        assert_eq!(storage.zone(ZoneId(0)), None);
    }

    #[test]
    fn collect_media() {
        let (sender, _receiver) = tokio::sync::broadcast::channel(10);
        let mut storage = Storage::new(sender);

        storage.on_ping(
            0,
            1,
            ClusterNodeInfo::Media(
                ClusterNodeGenericInfo {
                    addr: "addr".to_string(),
                    cpu: 11,
                    memory: 22,
                    disk: 33,
                },
                ClusterMediaInfo { live: 0, max: 100 },
            ),
            vec![],
        );
        storage.on_tick(0);

        assert_eq!(
            storage.zones(),
            vec![Zone {
                lat: 0.0,
                lon: 0.0,
                zone_id: 0,
                consoles: 0,
                gateways: 0,
                medias: 1,
                connectors: 0,
            }]
        );

        assert_eq!(
            storage.zone(ZoneId(0)),
            Some(ZoneDetails {
                lat: 0.0,
                lon: 0.0,
                consoles: vec![],
                gateways: vec![],
                medias: vec![MediaNode {
                    addr: "addr".to_string(),
                    node_id: 1,
                    cpu: 11,
                    memory: 22,
                    disk: 33,
                    conns: vec![],
                    live: 0,
                    max: 100,
                }],
                connectors: vec![]
            })
        );

        assert_eq!(storage.zone(ZoneId(1)), None);

        storage.on_tick(NODE_TIMEOUT);
        //after timeout should clear
        assert_eq!(storage.zones(), vec![]);
        assert_eq!(storage.zone(ZoneId(0)), None);
    }

    #[test]
    fn collect_connector() {
        let (sender, _receiver) = tokio::sync::broadcast::channel(10);
        let mut storage = Storage::new(sender);

        storage.on_ping(
            0,
            1,
            ClusterNodeInfo::Connector(ClusterNodeGenericInfo {
                addr: "addr".to_string(),
                cpu: 11,
                memory: 22,
                disk: 33,
            }),
            vec![],
        );
        storage.on_tick(0);

        assert_eq!(
            storage.zones(),
            vec![Zone {
                lat: 0.0,
                lon: 0.0,
                zone_id: 0,
                consoles: 0,
                gateways: 0,
                medias: 0,
                connectors: 1,
            }]
        );

        assert_eq!(
            storage.zone(ZoneId(0)),
            Some(ZoneDetails {
                lat: 0.0,
                lon: 0.0,
                consoles: vec![],
                gateways: vec![],
                medias: vec![],
                connectors: vec![ConnectorNode {
                    addr: "addr".to_string(),
                    node_id: 1,
                    cpu: 11,
                    memory: 22,
                    disk: 33,
                    conns: vec![],
                }]
            })
        );

        assert_eq!(storage.zone(ZoneId(1)), None);

        storage.on_tick(NODE_TIMEOUT);
        //after timeout should clear
        assert_eq!(storage.zones(), vec![]);
        assert_eq!(storage.zone(ZoneId(0)), None);
    }
}
