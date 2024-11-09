use media_server_protocol::media::MediaPacket;

mod vpx_demuxer;
mod vpx_writer;

pub use vpx_demuxer::*;
pub use vpx_writer::*;

pub trait CodecWriter {
    fn push_media(&mut self, pkt_ms: u64, pkt: MediaPacket);
}
