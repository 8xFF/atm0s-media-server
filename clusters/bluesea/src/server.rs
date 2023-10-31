use std::sync::Arc;

use cluster::Cluster;
use runner::{
    convert_enum, ChannelSourceHashmapReal, KeyValueBehavior, KeyValueBehaviorEvent, KeyValueHandlerEvent, KeyValueSdk, LayersSpreadRouterSyncBehavior, LayersSpreadRouterSyncBehaviorEvent,
    LayersSpreadRouterSyncHandlerEvent, ManualBehavior, ManualBehaviorConf, ManualBehaviorEvent, ManualHandlerEvent, NetworkPlane, NetworkPlaneConfig, NodeAddr, NodeAddrBuilder, NodeId, Protocol,
    PubsubSdk, PubsubServiceBehaviour, PubsubServiceBehaviourEvent, PubsubServiceHandlerEvent, SharedRouter, SystemTimer, UdpTransport,
};

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

pub struct ServerBlueseaConfig {
    pub neighbours: Vec<NodeAddr>,
}

pub struct ServerBluesea {
    join_handler: Option<async_std::task::JoinHandle<()>>,
    pubsub_sdk: PubsubSdk<NodeBehaviorEvent, NodeHandleEvent>,
    kv_sdk: KeyValueSdk,
}

impl ServerBluesea {
    pub async fn new(node_id: NodeId, config: ServerBlueseaConfig) -> Self {
        let node_addr_builder = Arc::new(NodeAddrBuilder::default());
        node_addr_builder.add_protocol(Protocol::P2p(node_id));
        let transport = Box::new(UdpTransport::new(node_id, 50000 + node_id as u16, node_addr_builder.clone()).await);
        let timer = Arc::new(SystemTimer());

        log::info!("[ServerBluesea] node addr: {}", node_addr_builder.addr());

        let router = SharedRouter::new(node_id);
        let manual = ManualBehavior::new(ManualBehaviorConf {
            neighbours: config.neighbours,
            timer: timer.clone(),
        });

        let router_sync_behaviour = LayersSpreadRouterSyncBehavior::new(router.clone());
        let (kv_behaviour, kv_sdk) = KeyValueBehavior::new(node_id, timer.clone(), 3000);
        let (pubsub_behavior, pubsub_sdk) = PubsubServiceBehaviour::new(node_id, Box::new(ChannelSourceHashmapReal::new(kv_sdk.clone(), node_id)));

        let mut plane = NetworkPlane::<NodeBehaviorEvent, NodeHandleEvent>::new(NetworkPlaneConfig {
            local_node_id: node_id,
            tick_ms: 1000,
            behavior: vec![Box::new(pubsub_behavior), Box::new(kv_behaviour), Box::new(router_sync_behaviour), Box::new(manual)],
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

impl Cluster<endpoint::BlueseaClusterEndpoint> for ServerBluesea {
    fn build(&mut self, room_id: &str, peer_id: &str) -> endpoint::BlueseaClusterEndpoint {
        endpoint::BlueseaClusterEndpoint::new(room_id, peer_id, self.pubsub_sdk.clone(), self.kv_sdk.clone())
    }
}

impl Drop for ServerBluesea {
    fn drop(&mut self) {
        if let Some(join_handler) = self.join_handler.take() {
            async_std::task::spawn(async move {
                join_handler.cancel().await;
            });
        }
    }
}
