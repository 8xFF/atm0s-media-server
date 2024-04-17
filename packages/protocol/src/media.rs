pub struct MediaPacket {
    pub pt: u8,
    pub ts: u32,
    pub seq: u64,
    pub marker: bool,
    pub nackable: bool,
    pub data: Vec<u8>,
}
