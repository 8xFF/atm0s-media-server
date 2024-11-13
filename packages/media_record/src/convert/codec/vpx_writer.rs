use std::io::{Seek, Write};

use media_server_protocol::media::MediaPacket;
use webm::mux::{AudioCodecId, AudioTrack, Segment, Track, VideoCodecId, VideoTrack, Writer};

use super::vpx_demuxer::VpxDemuxer;
use super::CodecWriter;

pub struct VpxWriter<W: Write + Seek> {
    webm: Option<Segment<Writer<W>>>,
    audio: Option<AudioTrack>,
    video: Option<(VideoTrack, VpxDemuxer)>,
    start_ts: u64,
    last_ts: u64,
}

impl<W: Write + Seek> VpxWriter<W> {
    pub fn new(writer: W, start_ts: u64) -> Self {
        let webm = Segment::new(Writer::new(writer)).expect("Should create webm");
        Self {
            webm: Some(webm),
            audio: None,
            video: None,
            start_ts,
            last_ts: start_ts,
        }
    }

    pub fn duration(&self) -> u64 {
        self.last_ts - self.start_ts
    }
}

impl<W: Write + Seek> CodecWriter for VpxWriter<W> {
    fn push_media(&mut self, pkt_ms: u64, pkt: MediaPacket) {
        let delta_ts = pkt_ms - self.start_ts;
        self.last_ts = pkt_ms;
        if pkt.meta.is_audio() {
            if self.audio.is_none() {
                if let Some(webm) = &mut self.webm {
                    self.audio = Some(webm.add_audio_track(48000, 2, None, AudioCodecId::Opus));
                } else {
                    log::warn!("Webm instant destroyed");
                    return;
                }
            }
            let audio = self.audio.as_mut().expect("Should have audio");
            audio.add_frame(&pkt.data, delta_ts * 1_000_000, true);
        } else {
            if self.video.is_none() {
                let codec = match &pkt.meta {
                    media_server_protocol::media::MediaMeta::Vp8 { .. } => VideoCodecId::VP8,
                    media_server_protocol::media::MediaMeta::Vp9 { .. } => VideoCodecId::VP9,
                    _ => panic!("Wrong codec, should be vp8 or vp9"),
                };
                let demuxer = VpxDemuxer::new();
                if let Some(webm) = &mut self.webm {
                    self.video = Some((webm.add_video_track(100, 100, None, codec), demuxer));
                } else {
                    log::warn!("Webm instant destroyed");
                    return;
                }
            }

            let (video, demuxer) = self.video.as_mut().expect("Should have video");
            if let Some((key, data)) = demuxer.push(pkt) {
                video.add_frame(&data, delta_ts * 1_000_000, key);
            }
        }
    }
}

impl<W: Write + Seek> Drop for VpxWriter<W> {
    fn drop(&mut self) {
        if let Some(webm) = self.webm.take() {
            if let Err(_e) = webm.try_finalize(Some(self.last_ts - self.start_ts)) {
                log::error!("Close VpxWriter failed");
            }
        }
    }
}
