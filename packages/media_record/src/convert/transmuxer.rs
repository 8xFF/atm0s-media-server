//!
//! Transmuxer download all raw record chunks and convert to some independent track video files.
//! The file is created at local at first then upload to s3, after upload to s3 successfully, it will be removed in local.
//! TODO: avoid using local file, may be we have way to do-it in-memory buffer then upload in-air to s3.
//!
use std::{collections::HashMap, path::PathBuf, str::FromStr, time::Duration};

use rusty_s3::S3Action;
use surf::Body;
use tokio::sync::mpsc::channel;

use crate::{storage::convert_s3_uri, RoomReader};

mod summary;
mod track_writer;

pub use summary::*;
use track_writer::*;

use super::RecordConvertOutputLocation;

pub struct RecordTransmuxer {
    in_s3: String,
    local_folder: String,
    out_s3: Option<String>,
}

impl RecordTransmuxer {
    pub fn new(in_s3: String, out: RecordConvertOutputLocation) -> Self {
        match out {
            RecordConvertOutputLocation::S3(s3) => Self {
                in_s3,
                out_s3: Some(s3),
                local_folder: format!("/tmp/media-record-transmuxer-{}", rand::random::<u64>()),
            },
            RecordConvertOutputLocation::Local(local) => Self {
                in_s3,
                out_s3: None,
                local_folder: local,
            },
        }
    }

    pub async fn convert(&self) -> Result<TransmuxSummary, String> {
        let (s3, credentials, s3_sub_folder) = convert_s3_uri(&self.in_s3).map_err(|e| e.to_string())?;
        let temp_folder = std::path::Path::new(&self.local_folder);
        std::fs::create_dir_all(temp_folder).map_err(|e| e.to_string())?;
        let mut record_summary = TransmuxSummary {
            peers: HashMap::new(),
            metadata_json: "".to_string(),
        };
        let room_reader = RoomReader::new(s3, credentials, &s3_sub_folder);
        let peers = room_reader.peers().await.map_err(|e| e.to_string())?;
        //we use channel to wait all sessions
        let (tx, mut rx) = channel(1);
        for peer in peers {
            let peer_id = peer.peer();
            log::info!("got peer {peer_id}");
            let sessions = peer.sessions().await.map_err(|e| e.to_string())?;
            for mut session in sessions {
                let tmp_folder = temp_folder.to_path_buf();
                let peer_id = peer_id.clone();
                let session_id = session.id();
                let session_prefix = format!("{}-{}-", peer_id, session_id);
                log::info!("got session {session_id}");
                let tx = tx.clone();
                tokio::spawn(async move {
                    log::info!("start session {session_id} loop");
                    let mut media = TrackWriter::new(tmp_folder, &session_prefix);
                    if let Err(e) = session.connect().await {
                        log::error!("connect session {session_id} failed: {e}");
                        //TODO send error to main
                        return;
                    }
                    while let Some(row) = session.recv().await {
                        log::debug!("push session {session_id} pkt {}", row.ts);
                        if let Some(event) = media.push(row) {
                            tx.send((peer_id.clone(), session_id, event)).await.expect("Should send to main");
                        }
                    }
                    log::info!("end session {session_id} loop");
                });
            }
        }
        drop(tx);

        while let Some((peer_id, session_id, event)) = rx.recv().await {
            let peer = record_summary.peers.entry(peer_id).or_default();
            let session = peer.sessions.entry(session_id).or_default();
            match event {
                track_writer::Event::TrackStart(name, kind, ts, path) => {
                    log::info!("track start {name} {kind} {ts} {path}");
                    let track: &mut TrackSummary = session.track.entry(name.into()).or_insert_with(|| TrackSummary { kind, timeline: vec![] });
                    track.timeline.push(TrackTimeline { path, start: ts, end: None });
                }
                track_writer::Event::TrackStop(name, _kind, ts) => {
                    log::info!("track stop {name} {ts}");
                    if let Some(track) = session.track.get_mut(name.as_str()) {
                        if let Some(timeline) = track.timeline.last_mut() {
                            if timeline.end.is_none() {
                                timeline.end = Some(ts);
                            } else {
                                log::warn!("timeline end not empty");
                            }
                        } else {
                            log::warn!("track stop but timeline not found");
                        }
                    } else {
                        log::warn!("track stop but track not found");
                    }
                }
            }
        }

        let summary_json = serde_json::to_string(&record_summary).expect("Should convert to json");

        if let Some(out_s3) = self.out_s3.as_ref() {
            let (s3, credentials, s3_sub_folder) = convert_s3_uri(out_s3).map_err(|e| e.to_string())?;
            let s3_output_path = PathBuf::from_str(&s3_sub_folder).map_err(|e| e.to_string())?;
            let summary_key = s3_output_path.join("summary.json").to_str().expect("Should convert to str").to_string();
            let summary_put_obj = s3.put_object(Some(&credentials), &summary_key);
            let summary_put_url = summary_put_obj.sign(Duration::from_secs(3600));
            surf::put(summary_put_url).body(Body::from_string(summary_json)).await.map_err(|e| e.to_string())?;

            for (_, peer) in record_summary.peers.iter() {
                for (_, session) in peer.sessions.iter() {
                    for (_, track) in session.track.iter() {
                        for timeline in track.timeline.iter() {
                            let path = temp_folder.join(&timeline.path);
                            let key = s3_output_path.join(&timeline.path).to_str().expect("Should convert").to_string();
                            let put_obj = s3.put_object(Some(&credentials), &key);
                            let put_url = put_obj.sign(Duration::from_secs(3600));
                            log::info!("upload track {:?}", path);
                            surf::put(put_url).body(Body::from_file(&path).await.map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
                            //remove file after upload success
                            tokio::fs::remove_file(&path).await.map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            record_summary.metadata_json = summary_key.to_string();
            Ok(record_summary)
        } else {
            let summary_out = temp_folder.join("summary.json").to_str().expect("Should convert path to str").to_string();
            std::fs::write(summary_out, &summary_json).map_err(|e| e.to_string())?;
            record_summary.metadata_json = summary_json;
            Ok(record_summary)
        }
    }
}
