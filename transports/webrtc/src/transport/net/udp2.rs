use async_std::net::UdpSocket;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub struct WebrtcUdpSocket {
    local_addr: SocketAddr,
    socket: UdpSocket,
}

impl WebrtcUdpSocket {
    pub async fn new(port: u16) -> Result<Self, std::io::Error> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("Should parse ip address");
        let socket = UdpSocket::bind(addr).await.expect("Should bind udp socket");

        Ok(Self {
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), socket.local_addr().expect("").port()),
            socket,
        })
    }

    pub fn proto(&self) -> str0m::net::Protocol {
        str0m::net::Protocol::Udp
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, std::net::SocketAddr, std::net::SocketAddr, str0m::net::Protocol)> {
        // let port = self.local_addr.port();
        self.socket.recv_from(buf).await.map(|(size, source)| (size, source, self.local_addr(), str0m::net::Protocol::Udp))
    }

    pub async fn send_to(&mut self, buf: &[u8], _from: std::net::SocketAddr, addr: std::net::SocketAddr) -> std::io::Result<usize> {
        self.socket.send_to(buf, addr).await
    }
}
