use std::io::{Seek, Write};

use media_server_protocol::media::MediaPacket;
use webm::mux::{AudioCodecId, AudioTrack, Segment, Track, VideoCodecId, VideoTrack, Writer};

use super::vpx_demuxer::VpxDemuxer;

pub struct VpxWriter<W: Write + Seek> {
    webm: Segment<Writer<W>>,
    audio: Option<AudioTrack>,
    video: Option<(VideoTrack, VpxDemuxer)>,
    start_ts: u64,
}

impl<W: Write + Seek> VpxWriter<W> {
    pub fn new(writer: W, start_ts: u64) -> Self {
        let mut webm = Segment::new(Writer::new(writer)).expect("Should create webm");
        //We must have audio before video
        let audio = Some(webm.add_audio_track(48000, 2, None, AudioCodecId::Opus));
        Self { webm, audio, video: None, start_ts }
    }

    pub fn push_opus(&mut self, pkt_ms: u64, pkt: MediaPacket) {
        let delta_ts = pkt_ms - self.start_ts;
        if self.audio.is_none() {
            self.audio = Some(self.webm.add_audio_track(48000, 2, None, AudioCodecId::Opus));
        }
        let audio = self.audio.as_mut().expect("Should have audio");
        audio.add_frame(&pkt.data, delta_ts * 1000_000, true);
    }

    pub fn push_vpx(&mut self, pkt_ms: u64, pkt: MediaPacket) {
        let delta_ts = pkt_ms - self.start_ts;
        if self.video.is_none() {
            let codec = match &pkt.meta {
                media_server_protocol::media::MediaMeta::Vp8 { .. } => VideoCodecId::VP8,
                media_server_protocol::media::MediaMeta::Vp9 { .. } => VideoCodecId::VP9,
                _ => panic!("Wrong codec, should be vp8 or vp9"),
            };
            let demuxer = VpxDemuxer::new();
            self.video = Some((self.webm.add_video_track(0, 0, None, codec), demuxer));
        }

        let (video, demuxer) = self.video.as_mut().expect("Should have video");
        if let Some((key, data)) = demuxer.push(pkt) {
            video.add_frame(&data, delta_ts * 1000_000, key);
        }
    }
}
