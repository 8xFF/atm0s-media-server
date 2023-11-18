use std::net::SocketAddr;
use udp_sas_async::async_std::UdpSocketSas;

pub struct WebrtcUdpSocket {
    local_addr: SocketAddr,
    socket: UdpSocketSas,
}

impl WebrtcUdpSocket {
    pub async fn new(port: u16) -> Result<Self, std::io::Error> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("Should parse ip address");
        let socket = UdpSocketSas::bind(addr).expect("Should bind udp socket");

        Ok(Self {
            local_addr: socket.local_addr(),
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
        let port = self.local_addr.port();
        self.socket
            .recv_sas(buf)
            .await
            .map(|(size, source, dest)| (size, source, SocketAddr::new(dest, port), str0m::net::Protocol::Udp))
    }

    pub async fn send_to(&mut self, buf: &[u8], from: std::net::SocketAddr, addr: std::net::SocketAddr) -> std::io::Result<usize> {
        self.socket.send_sas(buf, from.ip(), addr).await
    }
}
