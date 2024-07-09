use std::time::Duration;

use media_server_protocol::record::SessionRecordRow;
use rusty_s3::{actions::ListObjectsV2, Bucket, Credentials, S3Action};

use super::{chunk_reader::BodyWrap, RecordChunkReader};

pub struct SessionReader {
    s3: Bucket,
    credentials: Credentials,
    files: Vec<String>,
    current_chunk: Option<RecordChunkReader<BodyWrap>>,
}

impl SessionReader {
    pub async fn new(s3: Bucket, credentials: Credentials, path: &str) -> std::io::Result<Self> {
        let mut files = s3.list_objects_v2(Some(&credentials));
        files.with_prefix(path);
        let res = reqwest::get(files.sign(Duration::from_secs(3600)))
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, ""))?;
        let text = res.text().await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, ""))?;
        let parsed = ListObjectsV2::parse_response(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, ""))?;
        let mut files = parsed.contents.into_iter().map(|f| f.key).collect::<Vec<_>>();
        files.sort();
        files.reverse();

        Ok(Self {
            s3,
            credentials,
            files,
            current_chunk: None,
        })
    }

    pub async fn recv(&mut self) -> Option<SessionRecordRow> {
        loop {
            if self.current_chunk.is_none() {
                let first = self.files.pop()?;
                let first_chunk = self.s3.get_object(Some(&self.credentials), &first);
                let source = BodyWrap::get_uri(first_chunk.sign(Duration::from_secs(3600)).as_str()).await.ok()?;
                let chunk_reader = RecordChunkReader::new(source).await.ok()?;
                self.current_chunk = Some(chunk_reader);
            }

            let chunk = self.current_chunk.as_mut()?;
            let res = chunk.pop().await.ok()?;
            if res.is_none() {
                self.current_chunk = None;
                continue;
            }
            return res;
        }
    }
}
