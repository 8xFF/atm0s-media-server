use crate::cluster::ZoneId;

pub fn generate_gateway_zone_tag(zone: ZoneId) -> String {
    format!("gateway-zone-{}", zone.0)
}

pub const GATEWAY_RPC_PORT: u16 = 10000;
