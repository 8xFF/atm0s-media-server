use std::{net::SocketAddr, str::FromStr};

use async_std::net::UdpSocket;
use sdp_rs::{
    lines::{
        self,
        attribute::Rtpmap,
        common::{Addrtype, Nettype},
        connection::ConnectionAddress,
        media::{MediaType, ProtoType},
        Active, Connection, Media,
    },
    MediaDescription, SessionDescription, Time,
};
use transport::MediaPacket;

pub type Packet = Vec<u8>;

#[derive(Debug)]
pub enum RtpEngineError {
    InvalidSdp,
    MissingMedia,
}

pub struct RtpEngine {
    socket: UdpSocket,
}

impl RtpEngine {
    pub async fn new() -> Self {
        Self {
            socket: UdpSocket::bind("192.168.66.113:0").await.expect("Should open port"),
        }
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr().expect("Should get local addr")
    }

    pub async fn process_remote_sdp(&mut self, sdp: &str) -> Result<(), RtpEngineError> {
        let sdp = sdp_rs::SessionDescription::from_str(sdp).map_err(|_| RtpEngineError::InvalidSdp)?;
        let mut dest_addr = sdp.origin.unicast_address;
        let first = sdp.media_descriptions.first().ok_or(RtpEngineError::MissingMedia)?;
        if let Some(conn) = first.connections.first() {
            dest_addr = conn.connection_address.base;
        }
        let dest_port = first.media.port;
        self.socket.connect(SocketAddr::from((dest_addr, dest_port))).await;
        Ok(())
    }

    pub fn create_local_sdp(&mut self) -> String {
        let addr = self.socket.local_addr().expect("Should has local addr");
        let sdp = SessionDescription {
            version: lines::Version::V0,
            origin: lines::Origin {
                username: "z".to_string(),
                sess_id: "0".to_string(),
                sess_version: "441761850".to_string(),
                nettype: Nettype::In,
                addrtype: Addrtype::Ip4,
                unicast_address: addr.ip(),
            },
            session_name: lines::SessionName::new("bluesea".to_string()),
            session_info: None,
            uri: None,
            emails: vec![],
            phones: vec![],
            connection: Some(Connection {
                nettype: Nettype::In,
                addrtype: Addrtype::Ip4,
                connection_address: ConnectionAddress {
                    base: addr.ip(),
                    ttl: None,
                    numaddr: None,
                },
            }),
            bandwidths: vec![],
            times: vec1::vec1![Time {
                active: Active { start: 0, stop: 0 },
                repeat: vec![],
                zone: None,
            }],
            key: None,
            attributes: vec![],
            media_descriptions: vec![MediaDescription {
                media: Media {
                    media: MediaType::Audio,
                    port: addr.port(),
                    num_of_ports: None,
                    proto: ProtoType::RtpAvp,
                    fmt: "110 8 0 101".to_string(),
                },
                info: None,
                connections: vec![],
                bandwidths: vec![],
                key: None,
                attributes: vec![
                    lines::Attribute::Rtpmap(Rtpmap {
                        payload_type: 8,
                        encoding_name: "PCMA".to_string(),
                        clock_rate: 8000,
                        encoding_params: None,
                    }),
                    lines::Attribute::Rtpmap(Rtpmap {
                        payload_type: 101,
                        encoding_name: "telephone-event".to_string(),
                        clock_rate: 8000,
                        encoding_params: None,
                    }),
                    lines::Attribute::Sendrecv,
                    // lines::Attribute::Other("rtcp-mux".to_string(), None),
                ],
            }],
        };
        sdp.to_string()
    }

    pub async fn send(&self, pkt: Packet) {
        self.socket.send(&pkt).await.expect("Should send data");
    }

    pub async fn recv(&self) -> Option<Packet> {
        let mut buf = [0u8; 1500];
        let len = self.socket.recv(&mut buf).await.ok()?;
        Some(buf[..len].to_vec())
    }
}
