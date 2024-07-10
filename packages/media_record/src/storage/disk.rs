use std::collections::HashMap;

use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use super::{FileId, RecordFile, Storage};

pub struct DiskFile {
    path: String,
    file: Option<File>,
    id: FileId,
    len: usize,
    start_ts: Option<u64>,
    end_ts: Option<u64>,
}

pub struct DiskStorage {
    path: String,
    files: HashMap<FileId, DiskFile>,
}

impl RecordFile for DiskFile {
    fn id(&self) -> super::FileId {
        self.id
    }

    fn len(&self) -> usize {
        self.len
    }

    fn start_ts(&self) -> Option<u64> {
        self.start_ts
    }

    fn end_ts(&self) -> Option<u64> {
        self.end_ts
    }

    fn set_start_ts(&mut self, ts: u64) {
        self.start_ts = Some(ts);
    }

    fn set_end_ts(&mut self, ts: u64) {
        self.end_ts = Some(ts);
    }
}

impl AsyncRead for DiskFile {
    fn poll_read(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
        if self.file.is_none() {
            match std::fs::File::open(&self.path) {
                Ok(file) => {
                    self.file = Some(tokio::fs::File::from_std(file));
                }
                Err(e) => {
                    return std::task::Poll::Ready(Err(e));
                }
            }
        }
        let file = self.file.as_mut().expect("Should have file");
        std::pin::Pin::new(file).poll_read(cx, buf)
    }
}

impl AsyncWrite for DiskFile {
    fn poll_write(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
        if let Some(file) = self.file.as_mut() {
            std::pin::Pin::new(file).poll_write(cx, buf)
        } else {
            std::task::Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed on write")))
        }
    }

    fn poll_flush(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        if let Some(file) = self.file.as_mut() {
            std::pin::Pin::new(file).poll_flush(cx)
        } else {
            std::task::Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed on flush")))
        }
    }

    fn poll_shutdown(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        if let Some(mut file) = self.file.take() {
            std::pin::Pin::new(&mut file).poll_shutdown(cx)
        } else {
            std::task::Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed on shutdown")))
        }
    }
}

impl Drop for DiskFile {
    fn drop(&mut self) {
        let path = self.path.clone();
        let file = self.file.take();
        tokio::spawn(async move {
            if let Some(mut file) = file {
                let _ = file.shutdown().await;
                let _ = tokio::fs::remove_file(&path).await;
            }
        });
    }
}

impl DiskStorage {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            files: Default::default(),
        }
    }

    pub async fn copy_from_mem<F: RecordFile>(&self, old: F) -> std::io::Result<DiskFile> {
        let file_path = format!("{}/{}", self.path, old.id().0);
        let mut file = DiskFile {
            path: file_path.clone(),
            file: Some(tokio::fs::File::create(file_path).await?),
            id: old.id(),
            len: old.len(),
            start_ts: old.start_ts(),
            end_ts: old.end_ts(),
        };
        tokio::io::copy(&mut Box::pin(old), &mut file).await?;
        let _ = file.flush().await;
        let _ = file.shutdown().await;
        Ok(file)
    }
}

impl Storage<DiskFile> for DiskStorage {
    async fn push(&mut self, file: DiskFile) {
        self.files.insert(file.id(), file);
    }

    async fn pop(&mut self, file_id: super::FileId) -> Option<DiskFile> {
        self.files.remove(&file_id)
    }

    fn can_push(&self, _len: usize) -> bool {
        //TODO check disk size
        true
    }
}

#[cfg(test)]
mod test {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::storage::memory::MemoryFile;

    use super::DiskStorage;

    #[tokio::test]
    async fn simple() {
        let storage = DiskStorage::new("/tmp/");

        let mut mem_file = MemoryFile::default();
        mem_file.write(&[1, 2, 3, 4]).await.expect("should write");
        mem_file.write(&[5, 6, 7, 8, 9]).await.expect("should write");

        let mut file = storage.copy_from_mem(mem_file).await.expect("Should create file");

        let mut buf = [0; 10];
        let len = file.read(&mut buf).await.unwrap();
        assert_eq!(len, 9);
        assert_eq!(buf[0..9], [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
