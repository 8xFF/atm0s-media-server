mod sdp;
mod transport;
mod worker;

pub use transport::{RtpExtIn, RtpExtOut, VariantParams};
pub use worker::{MediaRtpWorker, RtpGroupIn, RtpGroupOut};

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive, derive_more::Display)]
#[repr(u32)]
pub enum RtpEngineError {
    InvalidSdp = 0x2000,
    InternalServerError = 0x2001,
}
