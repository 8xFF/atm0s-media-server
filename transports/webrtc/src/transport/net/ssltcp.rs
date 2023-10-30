use std::net::{IpAddr, SocketAddr};

use async_std::net::TcpStream;
use futures::{AsyncReadExt, AsyncWriteExt};

// Should this have a session id? The response doesn't have a
// certificate, so the hello should have a session id ?
const K_SSL_CLIENT_HELLO: [u8; 72] = [
    0x80, 0x46, // msg len
    0x01, // CLIENT_HELLO
    0x03, 0x01, // SSL 3.1
    0x00, 0x2d, // ciphersuite len
    0x00, 0x00, // session id len
    0x00, 0x10, // challenge len
    0x01, 0x00, 0x80, 0x03, 0x00, 0x80, 0x07, 0x00, 0xc0, // ciphersuites
    0x06, 0x00, 0x40, 0x02, 0x00, 0x80, 0x04, 0x00, 0x80, //
    0x00, 0x00, 0x04, 0x00, 0xfe, 0xff, 0x00, 0x00, 0x0a, //
    0x00, 0xfe, 0xfe, 0x00, 0x00, 0x09, 0x00, 0x00, 0x64, //
    0x00, 0x00, 0x62, 0x00, 0x00, 0x03, 0x00, 0x00, 0x06, //
    0x1f, 0x17, 0x0c, 0xa6, 0x2f, 0x00, 0x78, 0xfc, // challenge
    0x46, 0x55, 0x2e, 0xb1, 0x83, 0x39, 0xf1, 0xea, //
];

// This is a TLSv1 SERVER_HELLO message.
const K_SSL_SERVER_HELLO: [u8; 79] = [
    0x16, // handshake message
    0x03, 0x01, // SSL 3.1
    0x00, 0x4a, // message len
    0x02, // SERVER_HELLO
    0x00, 0x00, 0x46, // handshake len
    0x03, 0x01, // SSL 3.1
    0x42, 0x85, 0x45, 0xa7, 0x27, 0xa9, 0x5d, 0xa0, // server random
    0xb3, 0xc5, 0xe7, 0x53, 0xda, 0x48, 0x2b, 0x3f, //
    0xc6, 0x5a, 0xca, 0x89, 0xc1, 0x58, 0x52, 0xa1, //
    0x78, 0x3c, 0x5b, 0x17, 0x46, 0x00, 0x85, 0x3f, //
    0x20, // session id len
    0x0e, 0xd3, 0x06, 0x72, 0x5b, 0x5b, 0x1b, 0x5f, // session id
    0x15, 0xac, 0x13, 0xf9, 0x88, 0x53, 0x9d, 0x9b, //
    0xe8, 0x3d, 0x7b, 0x0c, 0x30, 0x32, 0x6e, 0x38, //
    0x4d, 0xa2, 0x75, 0x57, 0x41, 0x6c, 0x34, 0x5c, //
    0x00, 0x04, // RSA/RC4-128/MD5
    0x00, // null compression
];

struct SsltcpStream {
    stream: TcpStream,
    addr: SocketAddr,
    local: SocketAddr,
    handshake: bool,
    buf: Vec<u8>,
    buf_offset: usize,
    buf_len: usize,
    send_buf: Vec<u8>,
}

impl SsltcpStream {
    pub async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.buf_offset < self.buf_len {
                if self.buf_offset + 2 > self.buf_len {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid data"));
                }

                //get frame size form first 2 bytes from buf + offset
                let n = u16::from_be_bytes([self.buf[self.buf_offset], self.buf[self.buf_offset + 1]]) as usize;
                if self.buf_offset + n + 2 > self.buf_len {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid data"));
                }

                log::debug!("    => frame {}", n);

                self.buf_offset += 2;
                buf[0..n].copy_from_slice(&self.buf[self.buf_offset..self.buf_offset + n]);
                self.buf_offset += n;
                return Ok(n);
            }

            let n = self.stream.read(&mut self.buf).await?;
            if n == 0 {
                return Ok(0);
            }

            if !self.handshake {
                if self.buf[0..n].eq(&K_SSL_CLIENT_HELLO) {
                    self.stream.write(&K_SSL_SERVER_HELLO).await?;
                    self.handshake = true;
                } else {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid data"));
                }
            } else {
                self.buf_len = n;
                self.buf_offset = 0;
                log::debug!("received {}", n);
                continue;
            }
        }
    }

    pub async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !self.handshake {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "Waiting for handshake"));
        }
        self.send_buf[0..2].copy_from_slice(&(buf.len() as u16).to_be_bytes());
        self.send_buf[2..2 + buf.len()].copy_from_slice(buf);
        self.stream.write(&self.send_buf[0..2 + buf.len()]).await
    }
}

pub struct WebrtcSsltcpListener {
    async_listener: async_std::net::TcpListener,
    local_addr: SocketAddr,
    socket: Option<SsltcpStream>,
}

impl WebrtcSsltcpListener {
    pub async fn new(port: u16) -> Result<Self, std::io::Error> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("Should parse ip address");
        let async_socket = async_std::net::TcpListener::bind(addr).await?;

        Ok(Self {
            local_addr: async_socket.local_addr().expect("Should has local port"),
            async_listener: async_socket,
            socket: None,
        })
    }

    pub fn proto(&self) -> str0m::net::Protocol {
        str0m::net::Protocol::SslTcp
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, std::net::SocketAddr, std::net::SocketAddr, str0m::net::Protocol)> {
        loop {
            if let Some(stream) = self.socket.as_mut() {
                let n = stream.read(buf).await?;
                if n == 0 {
                    self.socket = None;
                    continue;
                }
                return Ok((n, stream.addr.clone(), stream.local.clone(), str0m::net::Protocol::SslTcp));
            } else {
                let (stream, addr) = self.async_listener.accept().await?;
                self.local_addr = stream.local_addr().expect("Should has local port");
                log::info!("[SslTcp] New connection from {} => {}", addr, self.local_addr);
                self.socket = Some(SsltcpStream {
                    local: stream.local_addr().expect("Should has local port"),
                    stream,
                    addr,
                    handshake: false,
                    buf: vec![0; 1 << 16],
                    buf_offset: 0,
                    buf_len: 0,
                    send_buf: vec![0; 1500],
                });
            }
        }
    }

    pub async fn send_to(&mut self, buf: &[u8], _addr: std::net::SocketAddr) -> std::io::Result<usize> {
        if let Some(socket) = &mut self.socket {
            socket.write(buf).await
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "Not connected"))
        }
    }
}
