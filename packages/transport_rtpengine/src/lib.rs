mod transport;
mod worker;

pub use transport::{ExtIn, ExtOut};
pub use worker::{GroupInput, GroupOutput, MediaWorkerRtpEngine, RtpEngineSession};

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive, derive_more::Display)]
#[repr(u32)]
pub enum RtpEngineError {
    InvalidSdp = 0x2000,
    InternalServerError = 0x2001,
    SdpConnectionNotFound = 0x2002,
    SdpMediaNotFound = 0x2003,
}
