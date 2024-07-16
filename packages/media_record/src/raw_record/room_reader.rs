use std::time::Duration;

use rusty_s3::{actions::ListObjectsV2, Bucket, Credentials, S3Action};

use super::peer_reader::PeerReader;

pub struct RoomReader {
    s3: Bucket,
    credentials: Credentials,
    path: String,
}

impl RoomReader {
    pub fn new(s3: Bucket, credentials: Credentials, path: &str) -> Self {
        log::info!("create room reader {path}");
        Self {
            s3,
            credentials,
            path: path.to_owned(),
        }
    }

    pub async fn peers(&self) -> std::io::Result<Vec<PeerReader>> {
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
                let peer = parts[parts.len() - 2];
                PeerReader::new(self.s3.clone(), self.credentials.clone(), peer, &f.prefix)
            })
            .collect::<Vec<_>>())
    }
}
