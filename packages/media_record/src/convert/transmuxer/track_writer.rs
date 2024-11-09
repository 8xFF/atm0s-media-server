use std::{collections::HashMap, fs::File, path::PathBuf};

use media_server_protocol::{
    endpoint::{TrackMeta, TrackName},
    media::MediaKind,
    record::{SessionRecordEvent, SessionRecordRow},
    transport::RemoteTrackId,
};

use crate::convert::codec::{CodecWriter, VpxWriter};

pub enum Event {
    TrackStart(TrackName, MediaKind, u64, String),
    TrackStop(TrackName, MediaKind, u64),
}

pub struct TrackWriter {
    folder: PathBuf,
    prefix: String,
    tracks_meta: HashMap<RemoteTrackId, (TrackName, TrackMeta)>,
    tracks_writer: HashMap<RemoteTrackId, Box<dyn CodecWriter + Send>>,
}

impl TrackWriter {
    pub fn new(folder: PathBuf, prefix: &str) -> Self {
        log::info!("new session media writer {folder:?}/{prefix}");
        Self {
            folder,
            prefix: prefix.to_string(),
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
                let (name, meta) = self.tracks_meta.remove(&id)?;
                self.tracks_writer.remove(&id)?;
                Some(Event::TrackStop(name, meta.kind, event.ts))
            }
            SessionRecordEvent::TrackMedia(id, media) => {
                // We allow clippy::map_entry because the suggestion provided by clippy has a bug:
                // cannot borrow `*self` as mutable more than once at a time
                // There is a open Issue on the Rust Clippy GitHub Repo:
                // https://github.com/rust-lang/rust-clippy/issues/11976
                #[allow(clippy::map_entry)]
                let out = if !self.tracks_writer.contains_key(&id) {
                    if let Some((name, meta)) = self.tracks_meta.get(&id) {
                        let (file_name, writer): (String, Box<dyn CodecWriter + Send>) = match &media.meta {
                            media_server_protocol::media::MediaMeta::Opus { .. } => {
                                let file_name = format!("{}-opus-{}-{}.webm", self.prefix, name, event.ts);
                                let file_path = self.folder.join(&file_name);
                                log::info!("create writer for track {name} => file {file_path:?}");
                                let writer = Box::new(VpxWriter::new(File::create(&file_path).unwrap(), event.ts));
                                (file_name, writer)
                            }
                            media_server_protocol::media::MediaMeta::H264 { .. } => todo!(),
                            media_server_protocol::media::MediaMeta::Vp8 { .. } => {
                                let file_name = format!("{}-vp8-{}-{}.webm", self.prefix, name, event.ts);
                                let file_path = self.folder.join(&file_name);
                                log::info!("create writer for track {name} => file {file_path:?}");
                                let writer = Box::new(VpxWriter::new(File::create(&file_path).unwrap(), event.ts));
                                (file_name, writer)
                            }
                            media_server_protocol::media::MediaMeta::Vp9 { .. } => {
                                let file_name = format!("{}-vp9-{}-{}.webm", self.prefix, name, event.ts);
                                let file_path = self.folder.join(&file_name);
                                log::info!("create writer for track {name} => file {file_path:?}");
                                let writer = Box::new(VpxWriter::new(File::create(&file_path).unwrap(), event.ts));
                                (file_name, writer)
                            }
                        };
                        log::info!("create writer for track {name} => file {file_name}");
                        self.tracks_writer.insert(id, writer);
                        Some(Event::TrackStart(name.clone(), meta.kind, event.ts, file_name))
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
