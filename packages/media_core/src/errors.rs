#[derive(Debug, num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(u16)]
pub enum EndpointErrors {
    EndpointNotInRoom = 0x0001,
    LocalTrackNotPinSource = 0x1001,
    RemoteTrack_ = 0x2001,
}

impl ToString for EndpointErrors {
    fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}
