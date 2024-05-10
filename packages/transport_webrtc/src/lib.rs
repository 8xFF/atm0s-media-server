mod media;
mod shared_port;
mod transport;
mod worker;

pub use transport::{ExtIn, ExtOut, Variant, VariantParams};
pub use worker::{GroupInput, GroupOutput, MediaWorkerWebrtc, WebrtcOwner};

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive, derive_more::Display)]
#[repr(u32)]
pub enum WebrtcError {
    SdpError = 0,
    Str0mError = 1,
    TrackNameNotFound = 2,
    TrackNotAttached = 3,
    TrackAlreadyAttached = 4,
}
