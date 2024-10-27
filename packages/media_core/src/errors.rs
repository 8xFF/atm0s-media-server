#[derive(Debug, num_enum::TryFromPrimitive, num_enum::IntoPrimitive, derive_more::Display)]
#[repr(u32)]
pub enum EndpointErrors {
    EndpointNotInRoom = 0x0001,
    LocalTrackNotPinSource = 0x1001,
    LocalTrackInvalidPriority = 0x1002,
    RemoteTrackInvalidPriority = 0x2001,
    RemoteTrackStopped = 0x2002,
    AudioMixerWrongMode = 0x3001,
}
