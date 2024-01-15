use std::{collections::HashMap, sync::Arc};

use async_std::channel::{bounded, Receiver, Sender};
use cluster::{atm0s_sdn::Timer, implement::NodeId, ClusterSessionUuid, MediaConnId, SessionTokenSigner, SessionTokenVerifier};
use metrics::{describe_counter, describe_gauge, gauge, increment_counter};
use parking_lot::RwLock;

const METRIC_SESSIONS_COUNT: &str = "media_server.sessions.count";
const METRIC_SESSIONS_LIVE: &str = "media_server.sessions.live";
const METRIC_SESSIONS_MAX: &str = "media_server.sessions.max";

#[cfg(feature = "connector")]
pub mod connector;
#[cfg(feature = "gateway")]
pub mod gateway;
#[cfg(feature = "rtmp")]
pub mod rtmp;
#[cfg(feature = "sip")]
pub mod sip;
#[cfg(feature = "token_generate")]
pub mod token_generate;
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
    session_counter: Arc<RwLock<u64>>,
    conn_counter: Arc<RwLock<u64>>,
    conns: Arc<RwLock<HashMap<String, (Sender<InternalControl>, PeerIdentity)>>>,
    peers: Arc<RwLock<HashMap<PeerIdentity, (Sender<InternalControl>, String)>>>,
    timer: Arc<dyn Timer>,
    token_verifier: Arc<dyn SessionTokenVerifier + Send + Sync>,
    token_signer: Arc<dyn SessionTokenSigner + Send + Sync>,
}

impl<InternalControl> Clone for MediaServerContext<InternalControl> {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id,
            conn_max: self.conn_max,
            session_counter: self.session_counter.clone(),
            conn_counter: self.conn_counter.clone(),
            conns: self.conns.clone(),
            peers: self.peers.clone(),
            timer: self.timer.clone(),
            token_verifier: self.token_verifier.clone(),
            token_signer: self.token_signer.clone(),
        }
    }
}

impl<InternalControl> MediaServerContext<InternalControl> {
    pub fn new(node_id: NodeId, conn_max: u64, timer: Arc<dyn Timer>, token_verifier: Arc<dyn SessionTokenVerifier + Send + Sync>, token_signer: Arc<dyn SessionTokenSigner + Send + Sync>) -> Self {
        Self {
            node_id,
            conn_max,
            session_counter: Arc::new(RwLock::new(0)),
            conn_counter: Arc::new(RwLock::new(0)),
            conns: Arc::new(RwLock::new(HashMap::new())),
            peers: Arc::new(RwLock::new(HashMap::new())),
            timer,
            token_verifier,
            token_signer,
        }
    }

    pub fn init_metrics(&self) {
        describe_counter!(METRIC_SESSIONS_COUNT, "Sum number of joined sessions");
        describe_gauge!(METRIC_SESSIONS_LIVE, "Current live sessions number");
        describe_gauge!(METRIC_SESSIONS_MAX, "Max live sessions number");

        gauge!(METRIC_SESSIONS_MAX, self.conn_max as f64);
    }

    /// Insert pair (Room, Peer) to store
    /// Return (event receiver, connection id, old pair sender), old pair sender can be used to force close old session
    pub fn create_peer(&self, room: &str, peer: &str, conn_id: Option<String>) -> (Receiver<InternalControl>, String, Option<Sender<InternalControl>>) {
        let peer = PeerIdentity::new(room, peer);
        let conn_id = conn_id.unwrap_or_else(|| self.generate_conn_id());
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
        gauge!(METRIC_SESSIONS_LIVE, self.peers.read().len() as f64);
        Some(tx)
    }

    pub fn close_conn(&self, conn_id: &str) -> Option<Sender<InternalControl>> {
        let (tx, peer_id) = self.conns.write().remove(conn_id)?;
        self.peers.write().remove(&peer_id);
        gauge!(METRIC_SESSIONS_LIVE, self.peers.read().len() as f64);
        Some(tx)
    }

    pub fn verifier(&self) -> Arc<dyn SessionTokenVerifier + Send + Sync> {
        self.token_verifier.clone()
    }

    fn signer(&self) -> Arc<dyn SessionTokenSigner + Send + Sync> {
        self.token_signer.clone()
    }

    pub fn generate_conn_id(&self) -> String {
        let mut counter = self.conn_counter.write();
        *counter += 1;
        self.token_signer.sign_conn_id(&MediaConnId {
            node_id: self.node_id,
            conn_id: *counter,
        })
    }

    pub fn generate_session_uuid(&self) -> u64 {
        let mut counter = self.session_counter.write();
        *counter += 1;
        let uuid = ClusterSessionUuid::new(self.node_id as u16, self.timer.now_ms() as u32, *counter as u16);
        uuid.to_u64()
    }
}
