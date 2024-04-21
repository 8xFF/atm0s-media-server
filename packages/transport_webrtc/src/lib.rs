mod shared_port;
mod transport;
mod utils;
mod worker;

pub use transport::{ExtIn, ExtOut, Variant, VariantParams};
pub use worker::{GroupInput, GroupOutput, MediaWorkerWebrtc, WebrtcOwner};

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(u16)]
pub enum WebrtcError {
    SdpError = 0,
    Str0mError = 1,
}

impl ToString for WebrtcError {
    fn to_string(&self) -> String {
        match self {
            Self::SdpError => "SdpError".to_string(),
            Self::Str0mError => "Str0mError".to_string(),
        }
    }
}
