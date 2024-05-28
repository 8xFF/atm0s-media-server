pub fn generate_gateway_zone_tag(zone: u32) -> String {
    format!("gateway-zone-{}", zone)
}

pub const GATEWAY_RPC_PORT: u16 = 10000;
