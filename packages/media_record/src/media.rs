use std::{collections::HashMap, fs::File};

use media_server_protocol::{
    record::{SessionRecordEvent, SessionRecordRow},
    transport::RemoteTrackId,
};
use vpx_writer::VpxWriter;

mod vpx_demuxer;
mod vpx_writer;

struct TrackWriter {
    writer: usize,
}

struct WriterContainer {
    writer: VpxWriter<File>,
    audio_inuse: bool,
    video_inuse: bool,
}

pub struct SessionMediaWriter {
    path: String,
    writers: Vec<WriterContainer>,
    tracks: HashMap<RemoteTrackId, TrackWriter>,
}

impl SessionMediaWriter {
    pub fn new(path: &str) -> Self {
        log::info!("new session media writer {path}");
        Self {
            path: path.to_string(),
            writers: vec![],
            tracks: HashMap::new(),
        }
    }

    fn get_free_writer_for(&mut self, ts: u64, is_audio: bool) -> usize {
        for (index, writer) in self.writers.iter().enumerate() {
            if (is_audio && !writer.audio_inuse) || (!is_audio && !writer.video_inuse) {
                return index;
            }
        }
        let index = self.writers.len();
        let path = format!("{}{}-{}.webm", self.path, index, ts);
        let writer = VpxWriter::new(File::create(&path).expect("Should open file"), ts);
        self.writers.push(WriterContainer {
            writer,
            audio_inuse: false,
            video_inuse: false,
        });
        index
    }

    pub fn push(&mut self, event: SessionRecordRow) {
        match event.event {
            SessionRecordEvent::TrackStarted(id, name, meta) => {
                log::info!("track {:?} started, name {name} meta {:?}", id, meta);
            }
            SessionRecordEvent::TrackStopped(id) => {
                log::info!("track {:?} stopped", id);
            }
            SessionRecordEvent::TrackMedia(id, media) => {
                if !self.tracks.contains_key(&id) {
                    let writer = self.get_free_writer_for(event.ts, media.meta.is_audio());
                    if media.meta.is_audio() {
                        self.writers[writer].audio_inuse = true;
                    } else {
                        self.writers[writer].video_inuse = true;
                    }
                    log::info!("write track {:?} to writer {writer}", id);
                    self.tracks.insert(id, TrackWriter { writer });
                }
                let track = self.tracks.get_mut(&id).expect("Should have track here");
                if media.meta.is_audio() {
                    self.writers[track.writer].writer.push_opus(event.ts, media);
                } else {
                    self.writers[track.writer].writer.push_vpx(event.ts, media);
                }
            }
            _ => {}
        }
    }
}
