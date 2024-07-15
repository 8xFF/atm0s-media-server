//!
//! Convert record util dowload all raw record chunks and convert to some independent track video files.
//! The file is created at local at first then upload to s3, after upload to s3 successfuly, it will be removed in local.
//! TODO: avoid using local file, may be we have way to do-it in-memory buffer then upload in-air to s3.
//!

use std::{collections::HashMap, time::Duration};

use clap::Parser;
use media_server_record::{RoomReader, SessionMediaWriter};
use media_server_utils::CustomUri;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use serde::{Deserialize, Serialize};
use surf::Body;
use tokio::sync::mpsc::channel;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Deserialize, Clone)]
struct S3Options {
    pub path_style: Option<bool>,
    pub region: Option<String>,
}

/// Record file converter for atm0s-media-server.
/// This tool allow convert room raw record to multiple webm files.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Http port
    #[arg(env, long)]
    in_uri: String,

    /// Http port
    #[arg(env, long)]
    out_uri: String,
}

fn convert_s3_uri(uri: &str) -> (Bucket, Credentials, String) {
    let s3_endpoint = CustomUri::<S3Options>::try_from(uri).expect("Should parse s3 uri");
    let url_style = if s3_endpoint.query.path_style == Some(true) {
        UrlStyle::Path
    } else {
        UrlStyle::VirtualHost
    };

    let s3_bucket = s3_endpoint.path[0].clone();
    let s3_sub_folder = s3_endpoint.path[1..].join("/");
    let s3 = Bucket::new(s3_endpoint.endpoint.parse().unwrap(), url_style, s3_bucket, s3_endpoint.query.region.unwrap_or("".to_string())).unwrap();
    let credentials = Credentials::new(s3_endpoint.username.expect("Should have s3 accesskey"), s3_endpoint.password.expect("Should have s3 secretkey"));
    (s3, credentials, s3_sub_folder)
}

#[derive(Serialize)]
struct TrackTimeline {
    path: String,
    start: u64,
    end: Option<u64>,
}

#[derive(Default, Serialize)]
struct TrackSummary {
    timeline: Vec<TrackTimeline>,
}

#[derive(Default, Serialize)]
struct SessionSummary {
    track: HashMap<String, TrackSummary>,
}

#[derive(Default, Serialize)]
struct PeerSummary {
    sessions: HashMap<u64, SessionSummary>,
}

#[derive(Default, Serialize)]
struct RecordSummary {
    peers: HashMap<String, PeerSummary>,
}

#[tokio::main]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();
    let (s3, credentials, s3_sub_folder) = convert_s3_uri(&args.in_uri);

    let mut record_summary = RecordSummary { peers: HashMap::new() };
    let room_reader = RoomReader::new(s3, credentials, &s3_sub_folder);
    let peers = room_reader.peers().await.unwrap();
    //we use channel to wait all sessions
    let (tx, mut rx) = channel(1);
    for peer in peers {
        let peer_id = peer.peer();
        log::info!("got peer {peer_id}");
        let sessions = peer.sessions().await.unwrap();
        for mut session in sessions {
            let peer_id = peer_id.clone();
            let session_id = session.id();
            log::info!("got session {session_id}");
            let tx = tx.clone();
            tokio::spawn(async move {
                log::info!("start session {session_id} loop");
                let mut media = SessionMediaWriter::new(&format!("{}-{}-", peer_id, session_id));
                session.connect().await.expect("Should connect session record folder");
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
            media_server_record::Event::TrackStart(name, ts, path) => {
                let track = session.track.entry(name.0).or_default();
                track.timeline.push(TrackTimeline { path, start: ts, end: None });
            }
            media_server_record::Event::TrackStop(name, ts) => {
                if let Some(track) = session.track.get_mut(&name.0) {
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

    let (s3, credentials, s3_sub_folder) = convert_s3_uri(&args.out_uri);
    let out_folder = std::path::Path::new(&s3_sub_folder);

    let summary_json = serde_json::to_string(&record_summary).expect("Should convert to json");
    let summary_path = out_folder.join("summary.json");
    let summary_key = summary_path.to_str().expect("Should convert");
    let summary_put_obj = s3.put_object(Some(&credentials), summary_key);
    let summary_put_url = summary_put_obj.sign(Duration::from_secs(3600));
    surf::put(summary_put_url).body(Body::from_string(summary_json)).await.expect("Should upload summary to s3");

    for (_, peer) in record_summary.peers {
        for (_, session) in peer.sessions {
            for (_, track) in session.track {
                for timeline in track.timeline {
                    let path = out_folder.join(&timeline.path);
                    let key = path.to_str().expect("Should convert");
                    let put_obj = s3.put_object(Some(&credentials), key);
                    let put_url = put_obj.sign(Duration::from_secs(3600));
                    surf::put(put_url)
                        .body(Body::from_file(&timeline.path).await.expect("Should open file"))
                        .await
                        .expect("Should upload to s3");
                    //remove file after upload success
                    tokio::fs::remove_file(&timeline.path).await.expect("Should remove file after upload");
                }
            }
        }
    }
}
