#[derive(Debug, num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(u16)]
pub enum EndpointErrors {
    LocalTrackSwitchNotInRoom = 0x0000,
    LocalTrackSwitchNotPin = 0x0001,
}

impl ToString for EndpointErrors {
    fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}
