use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use async_std::net::UdpSocket;
use media_utils::ErrorDebugger;
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

use self::rtp::is_rtp;

mod audio_frame;
mod g711;
mod g711_decoder;
mod g711_encoder;
mod opus_decoder;
mod opus_encoder;
mod resample;
mod rtp;

#[derive(Debug)]
pub enum RtpEngineError {
    InvalidSdp,
    MissingMedia,
    SocketError,
}

pub struct RtpEngine {
    buf: [u8; 2048],
    buf2: [u8; 2048],
    socket: UdpSocket,
    socket_sync: std::net::UdpSocket,
    opus_frame: audio_frame::AudioFrameMono<960, 48000>,
    g711_frame: audio_frame::AudioFrameMono<160, 8000>,
    resampler: resample::Resampler,
    opus_encoder: opus_encoder::OpusEncoder,
    opus_decoder: opus_decoder::OpusDecoder,
    g711_encoder: g711_encoder::G711Encoder,
    g711_decoder: g711_decoder::G711Decoder,
}

impl RtpEngine {
    pub async fn new(bind_ip: IpAddr) -> Self {
        let socket_sync = std::net::UdpSocket::bind(SocketAddr::new(bind_ip, 0)).expect("Should open port");
        Self {
            buf: [0; 2048],
            buf2: [0; 2048],
            g711_frame: Default::default(),
            opus_frame: Default::default(),
            socket: socket_sync.try_clone().expect("Should clone udp socket").try_into().expect("Should convert to async socket"),
            socket_sync,
            opus_encoder: opus_encoder::OpusEncoder::new(),
            opus_decoder: opus_decoder::OpusDecoder::new(),
            g711_encoder: g711_encoder::G711Encoder::new(g711::G711Codec::Alaw),
            g711_decoder: g711_decoder::G711Decoder::new(g711::G711Codec::Alaw),
            resampler: resample::Resampler::new(),
        }
    }

    #[allow(unused)]
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
        self.socket.connect(SocketAddr::from((dest_addr, dest_port))).await.map_err(|e| {
            log::error!("[RtpEngine] connect to {dest_addr}:{dest_port} error {:?}", e);
            RtpEngineError::SocketError
        })?;
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
            session_name: lines::SessionName::new("atm0s".to_string()),
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
                    fmt: "8 101".to_string(),
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

    pub fn send(&mut self, pkt: MediaPacket) {
        self.opus_decoder.decode(&pkt.payload, &mut self.opus_frame);
        self.resampler.from_48k_to_8k(&self.opus_frame, &mut self.g711_frame);
        let size = self.g711_encoder.encode(&self.g711_frame, &mut self.buf).expect("Should encode");

        let rtp = rtp_rs::RtpPacketBuilder::new()
            .payload_type(8)
            .sequence(pkt.seq_no.into())
            .timestamp((pkt.time / 6).into())
            .payload(&self.buf[0..size])
            .build()
            .expect("Should build rtp packet");

        self.socket_sync.send(&rtp).log_error("Should send rtp packet");
    }

    pub async fn recv(&mut self) -> Option<MediaPacket> {
        loop {
            let len = self.socket.recv(&mut self.buf).await.ok()?;
            if is_rtp(&self.buf[0..len]) {
                //Check is RTP packet for avoiding RTCP packet
                if let Ok(rtp) = rtp_rs::RtpReader::new(&self.buf[0..len]) {
                    if rtp.version() == 2 && rtp.payload_type() == 8 {
                        self.g711_decoder.decode(rtp.payload(), &mut self.g711_frame);
                        self.resampler.from_8k_to_48k(&self.g711_frame, &mut self.opus_frame);
                        let size = self.opus_encoder.encode(&self.opus_frame, &mut self.buf2).expect("Should encode");

                        let mut pkt = MediaPacket::simple_audio(rtp.sequence_number().into(), rtp.timestamp().wrapping_mul(6), self.buf2[0..size].to_vec());
                        pkt.ext_vals.audio_level = Some(-30); //TODO calculate audio level
                        break Some(pkt);
                    }
                }
            }
        }
    }
}
