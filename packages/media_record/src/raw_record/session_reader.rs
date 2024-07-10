use std::time::Duration;

use media_server_protocol::record::SessionRecordRow;
use rusty_s3::{actions::ListObjectsV2, Bucket, Credentials, S3Action};

use super::{chunk_reader::BodyWrap, RecordChunkReader};

pub struct SessionReader {
    session_id: u64,
    s3: Bucket,
    credentials: Credentials,
    path: String,
    files: Vec<String>,
    current_chunk: Option<RecordChunkReader<BodyWrap>>,
}

impl SessionReader {
    pub fn new(s3: Bucket, credentials: Credentials, session_id: u64, path: &str) -> Self {
        log::info!("create session reader {path}");
        Self {
            session_id,
            s3,
            credentials,
            path: path.to_string(),
            files: vec![],
            current_chunk: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.session_id
    }

    pub fn path(&self) -> String {
        self.path.clone()
    }

    pub async fn connect(&mut self) -> std::io::Result<()> {
        let mut files = self.s3.list_objects_v2(Some(&self.credentials));
        files.with_prefix(&self.path);
        files.with_delimiter("/");
        let res = reqwest::get(files.sign(Duration::from_secs(3600)))
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let text = res.text().await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, ""))?;
        let parsed = ListObjectsV2::parse_response(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        self.files = parsed.contents.into_iter().map(|f| f.key).collect::<Vec<_>>();
        self.files.sort();
        self.files.reverse();

        log::info!("got files for session {} {:?}", self.session_id, self.files);

        Ok(())
    }

    pub async fn recv(&mut self) -> Option<SessionRecordRow> {
        loop {
            if self.current_chunk.is_none() {
                let first = self.files.pop()?;
                log::info!("switch to chunk {first}");
                let first_chunk = self.s3.get_object(Some(&self.credentials), &first);
                let source = BodyWrap::get_uri(first_chunk.sign(Duration::from_secs(3600)).as_str()).await.ok()?;
                let mut chunk_reader = RecordChunkReader::new(source).await.ok()?;
                chunk_reader.connect().await.ok()?;
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
