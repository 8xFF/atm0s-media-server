use std::sync::Arc;

use crate::Cluster;
use atm0s_sdn::{
    compose_transport, convert_enum, KeyValueBehavior, KeyValueBehaviorEvent, KeyValueHandlerEvent, KeyValueSdk, KeyValueSdkEvent, LayersSpreadRouterSyncBehavior, LayersSpreadRouterSyncBehaviorEvent,
    LayersSpreadRouterSyncHandlerEvent, ManualBehavior, ManualBehaviorConf, ManualBehaviorEvent, ManualHandlerEvent, NetworkPlane, NetworkPlaneConfig, NodeAddr, NodeAddrBuilder, NodeId, PubsubSdk,
    PubsubServiceBehaviour, PubsubServiceBehaviourEvent, PubsubServiceHandlerEvent, RpcBox, RpcEmitter, SharedRouter, SystemTimer, TcpTransport, UdpTransport,
};

use super::{endpoint, rpc::RpcEndpointSdn};

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

pub struct ServerSdnConfig {
    pub static_key: String,
    pub seeds: Vec<NodeAddr>,
}

compose_transport!(UdpTcpTransport, udp: UdpTransport, tcp: TcpTransport);

pub struct ServerSdn {
    node_id: NodeId,
    join_handler: Option<async_std::task::JoinHandle<()>>,
    pubsub_sdk: PubsubSdk,
    kv_sdk: KeyValueSdk,
    rpc_emitter: RpcEmitter,
}

impl ServerSdn {
    pub async fn new(node_id: NodeId, service_id: u8, config: ServerSdnConfig) -> (Self, RpcEndpointSdn) {
        let mut node_addr_builder = NodeAddrBuilder::new(node_id);
        let udp_socket = UdpTransport::prepare(50000 + node_id as u16, &mut node_addr_builder).await;
        let tcp_listener = TcpTransport::prepare(50000 + node_id as u16, &mut node_addr_builder).await;
        let secure = Arc::new(atm0s_sdn::StaticKeySecure::new(&config.static_key));
        let udp = UdpTransport::new(node_addr_builder.addr(), udp_socket, secure.clone());
        let tcp = TcpTransport::new(node_addr_builder.addr(), tcp_listener, secure);

        let transport = UdpTcpTransport::new(udp, tcp);
        let timer = Arc::new(SystemTimer());

        log::info!("[ServerAtm0s] node addr: {}", node_addr_builder.addr());

        let router = SharedRouter::new(node_id);
        let manual = ManualBehavior::new(ManualBehaviorConf {
            node_id,
            node_addr: node_addr_builder.addr(),
            local_tags: vec!["media-server".to_string()],
            connect_tags: vec!["inner-gateway".to_string()],
            seeds: config.seeds,
        });

        let mut rpc_box = RpcBox::new(node_id, service_id, timer.clone());
        let router_sync_behaviour = LayersSpreadRouterSyncBehavior::new(router.clone());
        let kv_sdk = KeyValueSdk::new();
        let kv_behaviour = KeyValueBehavior::new(node_id, 3000, Some(Box::new(kv_sdk.clone())));
        let (pubsub_behavior, pubsub_sdk) = PubsubServiceBehaviour::new(node_id, timer.clone());

        let mut plane = NetworkPlane::<NodeBehaviorEvent, NodeHandleEvent, NodeSdkEvent>::new(NetworkPlaneConfig {
            node_id,
            tick_ms: 1000,
            behaviors: vec![
                Box::new(rpc_box.behaviour()),
                Box::new(pubsub_behavior),
                Box::new(kv_behaviour),
                Box::new(router_sync_behaviour),
                Box::new(manual),
            ],
            transport: Box::new(transport),
            timer,
            router: Arc::new(router.clone()),
        });

        plane.started();

        let join_handler = async_std::task::spawn_local(async move {
            while let Ok(_) = plane.recv().await {}
            plane.stopped();
        });

        (
            Self {
                node_id,
                pubsub_sdk,
                kv_sdk,
                join_handler: Some(join_handler),
                rpc_emitter: rpc_box.emitter(),
            },
            RpcEndpointSdn { rpc_box },
        )
    }
}

impl Cluster<endpoint::ClusterEndpointSdn> for ServerSdn {
    fn node_id(&self) -> u32 {
        self.node_id
    }

    fn build(&mut self, room_id: &str, peer_id: &str) -> endpoint::ClusterEndpointSdn {
        endpoint::ClusterEndpointSdn::new(room_id, peer_id, self.pubsub_sdk.clone(), self.kv_sdk.clone(), self.rpc_emitter.clone())
    }
}

impl Drop for ServerSdn {
    fn drop(&mut self) {
        if let Some(join_handler) = self.join_handler.take() {
            async_std::task::spawn(async move {
                join_handler.cancel().await;
            });
        }
    }
}
