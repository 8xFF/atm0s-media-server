use std::sync::Arc;

use runner::{
    convert_enum, ChannelSourceHashmapReal, KeyValueBehavior, KeyValueBehaviorEvent, KeyValueHandlerEvent, KeyValueSdk, LayersSpreadRouterSyncBehavior, LayersSpreadRouterSyncBehaviorEvent,
    LayersSpreadRouterSyncHandlerEvent, ManualBehavior, ManualBehaviorConf, ManualBehaviorEvent, ManualHandlerEvent, NetworkPlane, NetworkPlaneConfig, NodeAddr, NodeAddrBuilder, NodeId, Protocol,
    PubsubSdk, PubsubServiceBehaviour, PubsubServiceBehaviourEvent, PubsubServiceHandlerEvent, SharedRouter, SystemTimer, UdpTransport,
};

#[derive(convert_enum::From, convert_enum::TryInto)]
enum NodeBehaviorEvent {
    Manual(ManualBehaviorEvent),
    LayersSpreadRouterSync(LayersSpreadRouterSyncBehaviorEvent),
    KeyValue(KeyValueBehaviorEvent),
    Pubsub(PubsubServiceBehaviourEvent),
}

#[derive(convert_enum::From, convert_enum::TryInto)]
enum NodeHandleEvent {
    Manual(ManualHandlerEvent),
    LayersSpreadRouterSync(LayersSpreadRouterSyncHandlerEvent),
    KeyValue(KeyValueHandlerEvent),
    Pubsub(PubsubServiceHandlerEvent),
}

pub struct ServerBlueseaConfig {
    neighbours: Vec<NodeAddr>,
}

pub struct ServerBluesea {
    pubsub_sdk: PubsubSdk<NodeBehaviorEvent, NodeHandleEvent>,
    kv_sdk: KeyValueSdk,
}

impl ServerBluesea {
    pub async fn new(node_id: NodeId, config: ServerBlueseaConfig) -> Self {
        let node_addr_builder = Arc::new(NodeAddrBuilder::default());
        node_addr_builder.add_protocol(Protocol::P2p(node_id));
        let transport = Box::new(UdpTransport::new(node_id, 50000 + node_id as u16, node_addr_builder.clone()).await);
        let timer = Arc::new(SystemTimer());

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

        async_std::task::spawn(async move {
            plane.started();
            while let Ok(_) = plane.recv().await {}
            plane.stopped();
        });

        Self { pubsub_sdk, kv_sdk }
    }
}
