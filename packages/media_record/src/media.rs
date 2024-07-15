use std::{collections::HashMap, fs::File};

use media_server_protocol::{
    endpoint::{TrackMeta, TrackName},
    media::MediaPacket,
    record::{SessionRecordEvent, SessionRecordRow},
    transport::RemoteTrackId,
};
use vpx_writer::VpxWriter;

mod vpx_demuxer;
mod vpx_writer;

trait TrackWriter {
    fn push_media(&mut self, pkt_ms: u64, pkt: MediaPacket);
}

pub enum Event {
    TrackStart(TrackName, u64, String),
    TrackStop(TrackName, u64),
}

pub struct SessionMediaWriter {
    path: String,
    tracks_meta: HashMap<RemoteTrackId, (TrackName, TrackMeta)>,
    tracks_writer: HashMap<RemoteTrackId, Box<dyn TrackWriter + Send>>,
}

impl SessionMediaWriter {
    pub fn new(path: &str) -> Self {
        log::info!("new session media writer {path}");
        Self {
            path: path.to_string(),
            tracks_meta: HashMap::new(),
            tracks_writer: HashMap::new(),
        }
    }

    pub fn push(&mut self, event: SessionRecordRow) -> Option<Event> {
        match event.event {
            SessionRecordEvent::TrackStarted(id, name, meta) => {
                log::info!("track {:?} started, name {name} meta {:?}", id, meta);
                self.tracks_meta.insert(id, (name, meta));
                None
            }
            SessionRecordEvent::TrackStopped(id) => {
                log::info!("track {:?} stopped", id);
                let (name, _) = self.tracks_meta.remove(&id)?;
                self.tracks_writer.remove(&id)?;
                Some(Event::TrackStop(name, event.ts))
            }
            SessionRecordEvent::TrackMedia(id, media) => {
                let out = if !self.tracks_writer.contains_key(&id) {
                    if let Some((name, _meta)) = self.tracks_meta.get(&id) {
                        let (file_path, writer): (String, Box<dyn TrackWriter + Send>) = match &media.meta {
                            media_server_protocol::media::MediaMeta::Opus { .. } => {
                                let file_path = format!("{}-opus-{}-{}.webm", self.path, name.0, event.ts);
                                let writer = Box::new(VpxWriter::new(File::create(&file_path).unwrap(), event.ts));
                                (file_path, writer)
                            }
                            media_server_protocol::media::MediaMeta::H264 { .. } => todo!(),
                            media_server_protocol::media::MediaMeta::Vp8 { .. } => {
                                let file_path = format!("{}-vp8-{}-{}.webm", self.path, name.0, event.ts);
                                let writer = Box::new(VpxWriter::new(File::create(&file_path).unwrap(), event.ts));
                                (file_path, writer)
                            }
                            media_server_protocol::media::MediaMeta::Vp9 { .. } => {
                                let file_path = format!("{}-vp9-{}-{}.webm", self.path, name.0, event.ts);
                                let writer = Box::new(VpxWriter::new(File::create(&file_path).unwrap(), event.ts));
                                (file_path, writer)
                            }
                        };
                        log::info!("create writer for track {name}");
                        self.tracks_writer.insert(id, writer);
                        Some(Event::TrackStart(name.clone(), event.ts, file_path))
                    } else {
                        log::warn!("missing track info for pkt  form track {:?}", id);
                        return None;
                    }
                } else {
                    None
                };
                let writer = self.tracks_writer.get_mut(&id).expect("Should have track here");
                writer.push_media(event.ts, media);
                out
            }
            _ => None,
        }
    }
}
