use std::collections::{HashMap, VecDeque};

use atm0s_sdn::{features::socket, NodeId};
use sans_io_runtime::Buffer;
use tokio::{
    select,
    sync::mpsc::{channel, unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender},
};

use super::vsocket::VirtualUdpSocket;

#[derive(Debug)]
pub struct NetworkPkt {
    pub _local_port: u16,
    pub remote: NodeId,
    pub remote_port: u16,
    pub data: Buffer,
    pub meta: u8,
}

pub struct VirtualNetwork {
    node_id: NodeId,
    in_rx: Receiver<socket::Event>,
    out_tx: Sender<socket::Control>,
    close_socket_tx: UnboundedSender<u16>,
    close_socket_rx: UnboundedReceiver<u16>,
    sockets: HashMap<u16, Sender<NetworkPkt>>,
    ports: VecDeque<u16>,
}

impl VirtualNetwork {
    pub fn new(node_id: NodeId) -> (Self, Sender<socket::Event>, Receiver<socket::Control>) {
        let (in_tx, in_rx) = channel(1000);
        let (out_tx, out_rx) = channel(1000);
        let (close_socket_tx, close_socket_rx) = unbounded_channel();

        (
            Self {
                node_id,
                in_rx,
                out_tx,
                close_socket_rx,
                close_socket_tx,
                sockets: HashMap::new(),
                ports: (0..60000).collect(),
            },
            in_tx,
            out_rx,
        )
    }

    pub async fn udp_socket(&mut self, port: u16) -> Option<VirtualUdpSocket> {
        //remove port from ports
        let port = if port > 0 {
            let index = self.ports.iter().position(|&x| x == port).expect("Should have port");
            self.ports.swap_remove_back(index);
            port
        } else {
            self.ports.pop_front()?
        };
        self.out_tx.send(socket::Control::Bind(port)).await.expect("Should send bind");
        let (tx, rx) = channel(100);
        self.sockets.insert(port, tx);
        Some(VirtualUdpSocket::new(self.node_id, port, self.out_tx.clone(), rx, self.close_socket_tx.clone()))
    }

    pub async fn recv(&mut self) -> Option<()> {
        select! {
            port = self.close_socket_rx.recv() => {
                let port = port.expect("Should have port");
                self.ports.push_back(port);
                self.sockets.remove(&port);
                self.out_tx.send(socket::Control::Unbind(port)).await.expect("Should send unbind");
                Some(())
            }
            event = self.in_rx.recv() => {
                let event = event?;
                match event {
                    socket::Event::RecvFrom(local_port, remote, remote_port, data, meta) => {
                        let pkt = NetworkPkt { data, _local_port: local_port, remote, remote_port, meta };
                        if let Some(socket_tx) = self.sockets.get(&local_port) {
                            if let Err(e) = socket_tx.try_send(pkt) {
                                log::error!("Send to socket {} error {:?}", local_port, e);
                            }
                        } else {
                            log::warn!("No socket for port {}", local_port);
                        }
                    },
                }
                Some(())
            }
        }
    }
}
