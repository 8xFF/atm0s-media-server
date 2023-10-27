use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use futures::{select, FutureExt};
use local_ip_address::list_afinet_netifas;
use str0m::net::Protocol;

pub(crate) mod ssltcp;
pub(crate) mod udp;

pub struct ComposeSocket {
    udp: udp::WebrtcUdpSocket,
    ssltcp: ssltcp::WebrtcSsltcpListener,
    ssltcp_buf: Vec<u8>,
}

impl ComposeSocket {
    pub async fn new(port: u16) -> Result<Self, std::io::Error> {
        Ok(Self {
            udp: udp::WebrtcUdpSocket::new(port)?,
            ssltcp: ssltcp::WebrtcSsltcpListener::new(port).await?,
            ssltcp_buf: vec![0; 2000],
        })
    }

    pub fn local_addrs(&self) -> Vec<(SocketAddr, Protocol)> {
        if let Ok(network_interfaces) = list_afinet_netifas() {
            let mut addrs = vec![];
            for (_name, ip) in network_interfaces {
                if ip.is_ipv4() {
                    addrs.push((SocketAddr::new(ip, self.udp.local_addr().port()), self.udp.proto()));
                    addrs.push((SocketAddr::new(ip, self.ssltcp.local_addr().port()), self.ssltcp.proto()));
                }
            }
            addrs
        } else {
            vec![
                (SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), self.udp.local_addr().port()), self.udp.proto()),
                (SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), self.ssltcp.local_addr().port()), self.ssltcp.proto()),
            ]
        }
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, std::net::SocketAddr, std::net::SocketAddr, str0m::net::Protocol)> {
        select! {
            res = self.udp.recv(buf).fuse() => {
                res.map(|(size, addr, dest, proto)| {
                    (size, addr, dest, proto)
                })
            }
            res = self.ssltcp.recv(&mut self.ssltcp_buf).fuse() => {
                res.map(|(size, addr, dest, proto)| {
                    //TODO avoid copy_from_slice after read
                    buf[..size].copy_from_slice(&self.ssltcp_buf[..size]);
                    (size, addr, dest, proto)
                })
            }
        }
    }

    pub async fn send_to(&mut self, buf: &[u8], proto: Protocol, from: SocketAddr, addr: std::net::SocketAddr) -> std::io::Result<usize> {
        match proto {
            Protocol::Udp => self.udp.send_to(buf, from, addr).await,
            Protocol::SslTcp => self.ssltcp.send_to(buf, addr).await,
            _ => Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsupported protocol")),
        }
    }
}
