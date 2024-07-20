use media_server_protocol::media::MediaPacket;
use rtp::packet::Packet;

#[derive(Default)]
pub struct MediaConverter {}

impl MediaConverter {
    pub fn convert(&self, rtp: Packet) -> Option<MediaPacket> {
        None
    }
}
