#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive, derive_more::Display)]
#[repr(u32)]
pub enum MediaServerError {
    GatewayRpcError = 0x00020001,
    InvalidConnId = 0x00020002,
    NodePoolEmpty = 0x00020003,
    MediaResError = 0x00020004,
    NotImplemented = 0x00020005,
}
