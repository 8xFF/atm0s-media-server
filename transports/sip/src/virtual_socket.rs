use std::{collections::HashMap, fmt::Debug, hash::Hash, net::SocketAddr};

use async_std::channel::{bounded, Receiver, Sender};

pub enum VirtualSocketError {
    ChannelFull,
    ChannelClosed,
}

pub struct VirtualSocket<ID: Debug + Clone, MSG> {
    id: ID,
    main_tx: Sender<(ID, Option<(Option<SocketAddr>, MSG)>)>,
    rx: Receiver<MSG>,
    closed: bool,
}

impl<ID: Debug + Clone, MSG> VirtualSocket<ID, MSG> {
    pub fn send_to(&self, dest: Option<SocketAddr>, msg: MSG) -> Result<(), VirtualSocketError> {
        self.main_tx.try_send((self.id.clone(), Some((dest, msg)))).map_err(|_| VirtualSocketError::ChannelFull)
    }

    pub async fn recv(&self) -> Result<MSG, VirtualSocketError> {
        self.rx.recv().await.map_err(|_| VirtualSocketError::ChannelClosed)
    }

    pub async fn close(&mut self) {
        self.closed = true;
        if let Err(e) = self.main_tx.send((self.id.clone(), None)).await {
            log::error!("[VirtualSocket {:?}] close error {:?}", self.id, e);
        }
    }
}

impl<ID: Debug + Clone, MSG> Drop for VirtualSocket<ID, MSG> {
    fn drop(&mut self) {
        if !self.closed {
            log::error!("[VirtualSocket {:?}] drop without close", self.id);
            if let Err(e) = self.main_tx.try_send((self.id.clone(), None)) {
                log::error!("[VirtualSocket {:?}] close error {:?}", self.id, e);
            }
        }
    }
}
pub struct VirtualSocketPlane<ID, MSG> {
    sockets: HashMap<ID, Sender<MSG>>,
    main_tx: Sender<(ID, Option<(Option<SocketAddr>, MSG)>)>,
    main_rx: Receiver<(ID, Option<(Option<SocketAddr>, MSG)>)>,
}

impl<ID, MSG> Default for VirtualSocketPlane<ID, MSG> {
    fn default() -> Self {
        let (main_tx, main_rx) = bounded(1000);
        Self {
            sockets: HashMap::new(),
            main_tx,
            main_rx,
        }
    }
}

impl<ID: Debug + Clone + Hash + Eq, MSG> VirtualSocketPlane<ID, MSG> {
    pub fn new_socket(&mut self, id: ID) -> VirtualSocket<ID, MSG> {
        log::info!("Create socket for {:?}", id);
        let (tx, rx) = bounded(1000);
        self.sockets.insert(id.clone(), tx);
        VirtualSocket {
            id,
            main_tx: self.main_tx.clone(),
            rx,
            closed: false,
        }
    }

    pub fn forward(&mut self, id: &ID, msg: MSG) -> Option<()> {
        let tx = self.sockets.get_mut(&id)?;
        tx.try_send(msg).ok()
    }

    pub async fn recv(&mut self) -> Option<(ID, Option<(Option<SocketAddr>, MSG)>)> {
        self.main_rx.recv().await.ok()
    }

    pub fn close_socket(&mut self, id: &ID) {
        log::info!("Close socket {:?}", id);
        self.sockets.remove(id);
    }
}
