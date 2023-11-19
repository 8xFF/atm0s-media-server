use std::sync::Arc;

use atm0s_sdn::{
    convert_enum, KeyValueBehavior, KeyValueBehaviorEvent, KeyValueHandlerEvent, KeyValueSdk, KeyValueSdkEvent, LayersSpreadRouterSyncBehavior, LayersSpreadRouterSyncBehaviorEvent,
    LayersSpreadRouterSyncHandlerEvent, ManualBehavior, ManualBehaviorConf, ManualBehaviorEvent, ManualHandlerEvent, NetworkPlane, NetworkPlaneConfig, NodeAddr, NodeAddrBuilder, NodeId, Protocol,
    PubsubSdk, PubsubServiceBehaviour, PubsubServiceBehaviourEvent, PubsubServiceHandlerEvent, SharedRouter, SystemTimer, UdpTransport,
};
use cluster::Cluster;

use crate::endpoint;

#[derive(convert_enum::From, convert_enum::TryInto)]
pub(crate) enum NodeBehaviorEvent {
    Manual(ManualBehaviorEvent),
    LayersSpreadRouterSync(LayersSpreadRouterSyncBehaviorEvent),
    KeyValue(KeyValueBehaviorEvent),
    Pubsub(PubsubServiceBehaviourEvent),
}

#[derive(convert_enum::From, convert_enum::TryInto)]
pub(crate) enum NodeHandleEvent {
    Manual(ManualHandlerEvent),
    LayersSpreadRouterSync(LayersSpreadRouterSyncHandlerEvent),
    KeyValue(KeyValueHandlerEvent),
    Pubsub(PubsubServiceHandlerEvent),
}

#[derive(convert_enum::From, convert_enum::TryInto)]
pub(crate) enum NodeSdkEvent {
    KeyValue(KeyValueSdkEvent),
}

pub struct ServerAtm0sConfig {
    pub neighbours: Vec<NodeAddr>,
}

pub struct ServerAtm0s {
    join_handler: Option<async_std::task::JoinHandle<()>>,
    pubsub_sdk: PubsubSdk,
    kv_sdk: KeyValueSdk,
}

impl ServerAtm0s {
    pub async fn new(node_id: NodeId, config: ServerAtm0sConfig) -> Self {
        let node_addr_builder = Arc::new(NodeAddrBuilder::default());
        node_addr_builder.add_protocol(Protocol::P2p(node_id));
        let transport = Box::new(UdpTransport::new(node_id, 50000 + node_id as u16, node_addr_builder.clone()).await);
        let timer = Arc::new(SystemTimer());

        log::info!("[ServerAtm0s] node addr: {}", node_addr_builder.addr());

        let router = SharedRouter::new(node_id);
        let manual = ManualBehavior::new(ManualBehaviorConf {
            node_id,
            neighbours: config.neighbours,
            timer: timer.clone(),
        });

        let router_sync_behaviour = LayersSpreadRouterSyncBehavior::new(router.clone());
        let kv_sdk = KeyValueSdk::new();
        let kv_behaviour = KeyValueBehavior::new(node_id, 3000, Some(Box::new(kv_sdk.clone())));
        let (pubsub_behavior, pubsub_sdk) = PubsubServiceBehaviour::new(node_id, timer.clone());

        let mut plane = NetworkPlane::<NodeBehaviorEvent, NodeHandleEvent, NodeSdkEvent>::new(NetworkPlaneConfig {
            node_id,
            tick_ms: 1000,
            behaviors: vec![Box::new(pubsub_behavior), Box::new(kv_behaviour), Box::new(router_sync_behaviour), Box::new(manual)],
            transport,
            timer,
            router: Arc::new(router.clone()),
        });

        let join_handler = async_std::task::spawn(async move {
            plane.started();
            while let Ok(_) = plane.recv().await {}
            plane.stopped();
        });

        Self {
            pubsub_sdk,
            kv_sdk,
            join_handler: Some(join_handler),
        }
    }
}

impl Cluster<endpoint::Atm0sClusterEndpoint> for ServerAtm0s {
    fn build(&mut self, room_id: &str, peer_id: &str) -> endpoint::Atm0sClusterEndpoint {
        endpoint::Atm0sClusterEndpoint::new(room_id, peer_id, self.pubsub_sdk.clone(), self.kv_sdk.clone())
    }
}

impl Drop for ServerAtm0s {
    fn drop(&mut self) {
        if let Some(join_handler) = self.join_handler.take() {
            async_std::task::spawn(async move {
                join_handler.cancel().await;
            });
        }
    }
}
