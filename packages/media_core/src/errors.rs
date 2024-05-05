#[derive(Debug, num_enum::TryFromPrimitive, num_enum::IntoPrimitive, derive_more::Display)]
#[repr(u32)]
pub enum EndpointErrors {
    EndpointNotInRoom = 0x0001,
    LocalTrackNotPinSource = 0x1001,
    RemoteTrack_ = 0x2001,
}
