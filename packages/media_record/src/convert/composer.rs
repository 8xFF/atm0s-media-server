use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
    fs::File,
};

use audio_mixer::AudioMixer;
use media_server_protocol::record::{SessionRecordEvent, SessionRecordRow};
use surf::Body;
use video_composer::VideoComposer;

use crate::{storage::convert_s3_uri, RoomReader, SessionReader};

use super::{CodecWriter, RecordConvertOutputLocation, VpxWriter};

mod audio_mixer;
mod video_composer;

#[derive(Debug, Clone)]
pub struct RecordComposerConfig {
    pub audio: bool,
    pub video: bool,
    pub output_relative: String,
    pub output: RecordConvertOutputLocation,
}

struct SessionWrapper {
    session: SessionReader,
    front_ts: u64,
}

impl SessionWrapper {
    pub fn id(&self) -> u64 {
        self.session.id()
    }

    pub async fn pop(&mut self) -> Option<SessionRecordRow> {
        let res = self.session.pop().await?;
        // we temp set the front ts to the last pop ts
        self.front_ts = res.ts;
        // we update the front ts to the next peek ts
        // incase the session is empty then next pop will get none
        if let Some(ts) = self.session.peek_ts().await {
            self.front_ts = ts;
        }
        Some(res)
    }
}

impl Eq for SessionWrapper {}

impl PartialEq for SessionWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.front_ts == other.front_ts && self.session.id() == other.session.id()
    }
}

impl PartialOrd for SessionWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SessionWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        self.front_ts.cmp(&other.front_ts)
    }
}

#[derive(Debug, Clone)]
pub struct RecordComposerResult {
    pub media_uri: String,
    pub duration_ms: u64,
}

pub struct RecordComposer {
    audio: bool,
    video: bool,
    in_s3: String,
    out_local_path: String,
    out_s3: Option<String>,
    out_relative: String,
    sessions: BinaryHeap<Reverse<SessionWrapper>>,
    audio_mixer: Option<AudioMixer>,
    video_composer: Option<VideoComposer>,
    track_writer: Option<VpxWriter<File>>,
}

impl RecordComposer {
    pub fn new(in_s3: String, cfg: RecordComposerConfig) -> Self {
        assert!(cfg.audio || cfg.video);
        match cfg.output {
            RecordConvertOutputLocation::S3(s3) => Self {
                audio: cfg.audio,
                video: cfg.video,
                in_s3,
                out_s3: Some(s3),
                out_local_path: format!("/tmp/media-record-{}.vpx", rand::random::<u64>()),
                sessions: Default::default(),
                audio_mixer: cfg.audio.then_some(AudioMixer::new()),
                video_composer: cfg.video.then_some(VideoComposer::default()),
                track_writer: None,
                out_relative: cfg.output_relative,
            },
            RecordConvertOutputLocation::Local(local_path) => Self {
                audio: cfg.audio,
                video: cfg.video,
                in_s3,
                out_s3: None,
                out_local_path: local_path,
                sessions: Default::default(),
                audio_mixer: cfg.audio.then_some(AudioMixer::new()),
                video_composer: cfg.video.then_some(VideoComposer::default()),
                track_writer: None,
                out_relative: cfg.output_relative,
            },
        }
    }

    pub async fn compose(mut self) -> Result<RecordComposerResult, String> {
        let (s3, credentials, s3_sub_folder) = convert_s3_uri(&self.in_s3).map_err(|e| e.to_string())?;

        let room_reader = RoomReader::new(s3, credentials, &s3_sub_folder);
        let peers = room_reader.peers().await.map_err(|e| e.to_string())?;
        log::info!("check room peers {:?}", peers.iter().map(|p| p.peer()).collect::<Vec<_>>());
        //we use channel to wait all sessions
        for peer in peers {
            let sessions = peer.sessions().await.map_err(|e| e.to_string())?;
            log::info!("check peer {} sessions {:?}", peer.peer(), sessions.iter().map(|s| s.id()).collect::<Vec<_>>());
            for mut session in sessions {
                session.connect().await.map_err(|e| e.to_string())?;
                let id = session.id();
                if let Some(peek) = session.peek_ts().await {
                    log::info!("session {} has first packet at {:?}", id, peek);
                    self.sessions.push(Reverse(SessionWrapper { session, front_ts: peek }));
                } else {
                    log::warn!("session {} is empty", session.id());
                }
            }
        }

        loop {
            let mut need_pop = false;
            if let Some(mut session) = self.sessions.peek_mut() {
                if let Some(pkt) = session.0.pop().await {
                    match pkt.event {
                        SessionRecordEvent::TrackStarted(remote_track_id, track_name, track_meta) => {
                            if (!self.audio && track_meta.kind.is_audio()) || (!self.video && track_meta.kind.is_video()) {
                                continue;
                            }

                            if track_meta.kind.is_audio() {
                                if let Some(mixer) = &mut self.audio_mixer {
                                    mixer.add_track(session.0.id(), remote_track_id, track_name, track_meta);
                                }
                            } else if track_meta.kind.is_video() {
                                if let Some(composer) = &mut self.video_composer {
                                    composer.add_track(session.0.id(), remote_track_id, track_name, track_meta);
                                }
                            }
                        }
                        SessionRecordEvent::TrackMedia(remote_track_id, media_packet) => {
                            if (!self.audio && media_packet.meta.is_audio()) || (!self.video && media_packet.meta.is_video()) {
                                continue;
                            }

                            let media = if media_packet.meta.is_audio() {
                                if let Some(mixer) = &mut self.audio_mixer {
                                    mixer.on_media(session.0.id(), remote_track_id, pkt.ts, media_packet)
                                } else {
                                    None
                                }
                            } else {
                                if let Some(composer) = &mut self.video_composer {
                                    composer.on_media(session.0.id(), remote_track_id, media_packet);
                                }
                                None
                            };

                            if let Some((ts, pkt)) = media {
                                if let Some(file) = self.track_writer.as_mut() {
                                    file.push_media(ts, pkt);
                                } else {
                                    log::info!("[RecodeComposer] creating output file {}", self.out_local_path);
                                    let mut file = VpxWriter::new(File::create(self.out_local_path.as_str()).map_err(|e| e.to_string())?, ts);
                                    file.push_media(ts, pkt);
                                    self.track_writer = Some(file);
                                }
                            }
                        }
                        SessionRecordEvent::TrackStopped(remote_track_id) => {
                            if let Some(mixer) = &mut self.audio_mixer {
                                mixer.remove_track(session.0.id(), remote_track_id);
                            }
                            if let Some(composer) = &mut self.video_composer {
                                composer.remove_track(session.0.id(), remote_track_id);
                            }
                        }
                        _ => {}
                    }
                } else {
                    need_pop = true;
                }
            } else {
                break;
            }

            if need_pop {
                self.sessions.pop();
            }
        }

        if let Some(mixer) = &mut self.audio_mixer {
            while let Some((ts, pkt)) = mixer.force_pop() {
                self.track_writer.as_mut().expect("must have track").push_media(ts, pkt);
            }
        }

        let track_writer = self.track_writer.take().ok_or("record empty".to_string())?;
        let duration_ms = track_writer.duration();

        if let Some(out_s3) = self.out_s3.take() {
            surf::put(&out_s3)
                .body(Body::from_file(&self.out_local_path).await.map_err(|e| e.to_string())?)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(RecordComposerResult {
            media_uri: self.out_relative,
            duration_ms,
        })
    }
}
