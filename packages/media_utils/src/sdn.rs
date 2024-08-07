pub fn node_zone_id(node: u32) -> u32 {
    node & 0xFFFFFF00
}
