use std::time::Duration;

use rusty_s3::{actions::ListObjectsV2, Bucket, Credentials, S3Action};

use crate::SessionReader;

pub struct PeerReader {
    peer: String,
    s3: Bucket,
    credentials: Credentials,
    path: String,
}

impl PeerReader {
    pub fn new(s3: Bucket, credentials: Credentials, peer: &str, path: &str) -> Self {
        log::info!("create peer reader {path}");
        Self {
            peer: peer.to_string(),
            s3,
            credentials,
            path: path.to_owned(),
        }
    }

    pub fn peer(&self) -> String {
        self.peer.clone()
    }

    pub async fn sessions(&self) -> std::io::Result<Vec<SessionReader>> {
        let mut files = self.s3.list_objects_v2(Some(&self.credentials));
        files.with_prefix(&self.path);
        files.with_delimiter("/");

        let res = reqwest::get(files.sign(Duration::from_secs(3600)))
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let text = res.text().await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let parsed = ListObjectsV2::parse_response(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(parsed
            .common_prefixes
            .into_iter()
            .map(|f| {
                let parts = f.prefix.split('/').collect::<Vec<_>>();
                let session_id: u64 = parts[parts.len() - 2].parse().expect("Should parse to session_id");
                SessionReader::new(self.s3.clone(), self.credentials.clone(), session_id, &f.prefix)
            })
            .collect::<Vec<_>>())
    }
}
