mod media;
mod shared_port;
mod transport;
mod worker;

pub use transport::{ExtIn, ExtOut, Variant, VariantParams};
pub use worker::{GroupInput, GroupOutput, MediaWorkerWebrtc, WebrtcOwner};

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive, derive_more::Display)]
#[repr(u32)]
pub enum WebrtcError {
    InvalidSdp = 0x2000,
    InternalServerError = 0x2001,
    RpcInvalidRequest = 0x2002,
    RpcTrackNameNotFound = 0x2003,
    RpcTrackNotAttached = 0x2004,
    RpcTrackAlreadyAttached = 0x2005,
    RpcEndpointNotFound = 0x2006,
    RpcTokenInvalid = 0x2007,
    RpcTokenRoomPeerNotMatch = 0x2008,
}
