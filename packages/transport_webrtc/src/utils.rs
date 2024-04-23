use media_server_protocol::media::MediaPacket;
use str0m::rtp::RtpPacket;

pub fn rtp_to_media_packet(rtp: RtpPacket) -> MediaPacket {
    MediaPacket {
        pt: *rtp.header.payload_type,
        ts: rtp.header.timestamp,
        seq: *rtp.seq_no,
        marker: rtp.header.marker,
        nackable: *rtp.header.payload_type != 111,
        data: rtp.payload,
    }
}
