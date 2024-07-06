use std::{
    collections::{HashMap, VecDeque},
    io::Write,
};

use tokio::io::{AsyncRead, AsyncWrite};

use super::{FileId, RecordFile, Storage};

pub struct MemoryFile {
    chunks: VecDeque<Vec<u8>>,
    id: FileId,
    len: usize,
    start_ts: Option<u64>,
    end_ts: Option<u64>,
}

pub struct MemoryStorage {
    files: HashMap<FileId, MemoryFile>,
    current_size: usize,
    max_size: usize,
}

impl Default for MemoryFile {
    fn default() -> Self {
        Self {
            chunks: VecDeque::new(),
            id: FileId(rand::random()),
            len: 0,
            start_ts: None,
            end_ts: None,
        }
    }
}

impl RecordFile for MemoryFile {
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

impl Write for MemoryFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.chunks.push_back(buf.to_vec());
        self.len += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.chunks.clear();
        self.len = 0;
        Ok(())
    }
}

impl AsyncRead for MemoryFile {
    fn poll_read(mut self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
        //TODO can pop multi times
        if let Some(first) = self.chunks.pop_front() {
            self.len -= first.len();
            buf.put_slice(&first);
            std::task::Poll::Ready(Ok(()))
        } else {
            std::task::Poll::Ready(Ok(()))
        }
    }
}

impl AsyncWrite for MemoryFile {
    fn poll_write(mut self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
        self.chunks.push_back(buf.to_vec());
        self.len += buf.len();
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

impl MemoryStorage {
    pub fn new(max_size: usize) -> Self {
        Self {
            files: Default::default(),
            current_size: 0,
            max_size,
        }
    }
}

impl Storage<MemoryFile> for MemoryStorage {
    async fn push(&mut self, file: MemoryFile) {
        self.current_size += file.len();
        self.files.insert(file.id(), file);
    }

    async fn pop(&mut self, file_id: super::FileId) -> Option<MemoryFile> {
        let file = self.files.remove(&file_id)?;
        self.current_size -= file.len();
        Some(file)
    }

    fn can_push(&self, len: usize) -> bool {
        self.current_size + len <= self.max_size
    }
}

#[cfg(test)]
mod test {
    use tokio::io::AsyncReadExt;

    use crate::storage::RecordFile;

    use super::MemoryFile;

    #[test]
    fn metadata() {
        let mut file = MemoryFile::default();
        assert_eq!(file.start_ts(), None);
        assert_eq!(file.end_ts(), None);

        file.set_start_ts(1);
        file.set_end_ts(2);
        assert_eq!(file.start_ts(), Some(1));
        assert_eq!(file.end_ts(), Some(2));
    }

    #[tokio::test]
    async fn simple_sync_write() {
        use std::io::Write;

        let mut file = MemoryFile::default();
        file.write(&[1, 2, 3, 4]).expect("should write");
        file.write(&[5, 6, 7, 8, 9]).expect("should write");

        let mut buf = [0; 10];
        let len = file.read(&mut buf).await.unwrap();
        assert_eq!(len, 4);
        assert_eq!(buf[0..4], [1, 2, 3, 4]);

        let len = file.read(&mut buf).await.unwrap();
        assert_eq!(len, 5);
        assert_eq!(buf[0..5], [5, 6, 7, 8, 9]);
    }

    #[tokio::test]
    async fn simple_asyncasync_write() {
        use tokio::io::AsyncWriteExt;

        let mut file = MemoryFile::default();
        file.write(&[1, 2, 3, 4]).await.expect("should write");
        file.write(&[5, 6, 7, 8, 9]).await.expect("should write");

        let mut buf = [0; 10];
        let len = file.read(&mut buf).await.unwrap();
        assert_eq!(len, 4);
        assert_eq!(buf[0..4], [1, 2, 3, 4]);

        let len = file.read(&mut buf).await.unwrap();
        assert_eq!(len, 5);
        assert_eq!(buf[0..5], [5, 6, 7, 8, 9]);
    }
}
