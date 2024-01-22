mod rtp_engine;
mod server;
mod sip;
mod transport_in;
mod transport_out;
mod virtual_socket;

pub use crate::transport_in::SipTransportIn;
pub use crate::transport_out::SipTransportOut;
pub use server::{SipServerSocket, SipServerSocketError, SipServerSocketMessage};
pub use sip::*;
pub use virtual_socket::{VirtualSocket, VirtualSocketContext, VirtualSocketError};

pub const LOCAL_TRACK_AUDIO_MAIN: u16 = 1;
pub const REMOTE_TRACK_AUDIO_MAIN: u16 = 2;
