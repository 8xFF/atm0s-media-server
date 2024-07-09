use std::fs::File;

use media_server_protocol::{
    media::MediaMeta,
    record::{SessionRecordEvent, SessionRecordRow},
};
use vpx_demuxer::VpxDemuxer;
use webm::mux::{AudioCodecId, AudioTrack, Segment, Track, VideoCodecId, VideoTrack, Writer};

mod vpx_demuxer;

pub struct ComposeSessionWebm {
    demuxer: VpxDemuxer,
    segment: Segment<Writer<File>>,
    audio: AudioTrack,
    video: VideoTrack,
    video2: VideoTrack,
    start_ts: Option<u64>,
}

impl ComposeSessionWebm {
    pub fn new() -> Self {
        let file = File::create("./record.webm").unwrap();
        let writer = Writer::new(file);
        let mut segment = Segment::new(writer).unwrap();

        let audio = segment.add_audio_track(48000, 2, Some(1), AudioCodecId::Opus);
        let video = segment.add_video_track(320, 240, Some(2), VideoCodecId::VP8);
        let video2 = segment.add_video_track(320, 240, Some(3), VideoCodecId::VP8);

        Self {
            start_ts: None,
            demuxer: VpxDemuxer::new(),
            segment,
            audio,
            video,
            video2,
        }
    }

    pub fn push(&mut self, event: SessionRecordRow) {
        if self.start_ts.is_none() {
            self.start_ts = Some(event.ts);
        }
        let delta_ts = event.ts - self.start_ts.expect("Should have start_ts");

        match event.event {
            SessionRecordEvent::TrackStarted(id, name, meta) => {}
            SessionRecordEvent::TrackStopped(id) => {}
            SessionRecordEvent::TrackMedia(id, media) => match media.meta {
                MediaMeta::Opus { .. } => {
                    self.audio.add_frame(&media.data, delta_ts * 1000000, true);
                }
                MediaMeta::H264 { key, profile, sim } => todo!(),
                MediaMeta::Vp8 { key, sim } => {
                    let should_process = if let Some(sim) = sim {
                        sim.spatial == 0
                    } else {
                        true
                    };

                    if !should_process {
                        return;
                    }

                    if let Some((is_key, _, frame)) = self.demuxer.push(key, media) {
                        log::info!("on vp8 frame {delta_ts} {is_key} {}", frame.len());
                        self.video.add_frame(&frame, delta_ts * 1000000, is_key);
                        self.video2.add_frame(&frame, delta_ts * 1000000, is_key);
                    }
                }
                MediaMeta::Vp9 { key, profile, svc } => todo!(),
            },
            _ => {}
        }
    }
}
