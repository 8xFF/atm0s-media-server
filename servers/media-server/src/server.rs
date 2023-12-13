use std::{collections::HashMap, sync::Arc};

use async_std::channel::{bounded, Receiver, Sender};
use cluster::{implement::NodeId, rpc::gateway::create_conn_id};
use metrics::{describe_counter, describe_gauge, gauge, increment_counter};
use parking_lot::RwLock;

const METRIC_SESSIONS_COUNT: &str = "media_server.sessions.count";
const METRIC_SESSIONS_LIVE: &str = "media_server.sessions.live";
const METRIC_SESSIONS_MAX: &str = "media_server.sessions.max";

#[cfg(feature = "gateway")]
pub mod gateway;
#[cfg(feature = "rtmp")]
pub mod rtmp;
#[cfg(feature = "sip")]
pub mod sip;
#[cfg(feature = "webrtc")]
pub mod webrtc;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct PeerIdentity {
    room: String,
    peer: String,
}

impl PeerIdentity {
    pub fn new(room: &str, peer: &str) -> Self {
        Self { room: room.into(), peer: peer.into() }
    }
}

pub struct MediaServerContext<InternalControl> {
    node_id: NodeId,
    conn_max: u64,
    counter: Arc<RwLock<u64>>,
    conns: Arc<RwLock<HashMap<String, (Sender<InternalControl>, PeerIdentity)>>>,
    peers: Arc<RwLock<HashMap<PeerIdentity, (Sender<InternalControl>, String)>>>,
}

impl<InternalControl> Clone for MediaServerContext<InternalControl> {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id,
            conn_max: self.conn_max,
            counter: self.counter.clone(),
            conns: self.conns.clone(),
            peers: self.peers.clone(),
        }
    }
}

impl<InternalControl> MediaServerContext<InternalControl> {
    pub fn new(node_id: NodeId, conn_max: u64) -> Self {
        Self {
            node_id,
            conn_max,
            counter: Arc::new(RwLock::new(0)),
            conns: Arc::new(RwLock::new(HashMap::new())),
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn init_metrics(&self) {
        describe_counter!(METRIC_SESSIONS_COUNT, "Sum number of joined sessions");
        describe_gauge!(METRIC_SESSIONS_LIVE, "Current live sessions number");

        gauge!(METRIC_SESSIONS_MAX, self.conn_max as f64);
    }

    /// Insert pair (Room, Peer) to store
    /// Return (event receiver, connection id, old pair sender), old pair sender can be used to force close old session
    pub fn create_peer(&self, room: &str, peer: &str) -> (Receiver<InternalControl>, String, Option<Sender<InternalControl>>) {
        let peer = PeerIdentity::new(room, peer);
        let conn_id = self.generate_conn_id();
        let (tx, rx) = bounded(10);
        let mut peers = self.peers.write();
        let mut conns = self.conns.write();
        let old_conn = peers.insert(peer.clone(), (tx.clone(), conn_id.clone()));
        conns.insert(conn_id.clone(), (tx, peer.clone()));

        increment_counter!(METRIC_SESSIONS_COUNT);
        gauge!(METRIC_SESSIONS_LIVE, peers.len() as f64);

        (rx, conn_id, old_conn.map(|(tx, _)| tx))
    }

    pub fn get_conn(&self, conn_id: &str) -> Option<Sender<InternalControl>> {
        let (tx, _peer_id) = self.conns.read().get(conn_id)?.clone();
        Some(tx.clone())
    }

    pub fn conns_live(&self) -> u64 {
        self.peers.write().len() as u64
    }

    pub fn conns_max(&self) -> u64 {
        self.conn_max
    }

    #[allow(unused)]
    pub fn close_peer(&self, room: &str, peer: &str) -> Option<Sender<InternalControl>> {
        let peer = PeerIdentity::new(room, peer);
        let (tx, conn_id) = self.peers.write().remove(&peer)?;
        self.conns.write().remove(&conn_id);
        Some(tx)
    }

    pub fn close_conn(&self, conn_id: &str) -> Option<Sender<InternalControl>> {
        let (tx, peer_id) = self.conns.write().remove(conn_id)?;
        self.peers.write().remove(&peer_id);
        Some(tx)
    }

    fn generate_conn_id(&self) -> String {
        let mut counter = self.counter.write();
        *counter += 1;
        create_conn_id(self.node_id, *counter)
    }
}
