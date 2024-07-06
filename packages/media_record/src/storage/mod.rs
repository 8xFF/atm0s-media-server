use disk::{DiskFile, DiskStorage};
use memory::{MemoryFile, MemoryStorage};
use tokio::io::{AsyncRead, AsyncWrite};

pub mod disk;
pub mod memory;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct FileId(pub u64);

pub trait Storage<F: RecordFile> {
    async fn push(&mut self, file: F);
    async fn pop(&mut self, file_id: FileId) -> Option<F>;
    fn can_push(&self, len: usize) -> bool;
}

pub trait RecordFile: AsyncRead + AsyncWrite {
    fn id(&self) -> FileId;
    fn len(&self) -> usize;
    fn set_start_ts(&mut self, ts: u64);
    fn set_end_ts(&mut self, ts: u64);
    fn start_ts(&self) -> Option<u64>;
    fn end_ts(&self) -> Option<u64>;
}

pub enum HybridFile {
    Mem(MemoryFile),
    Disk(DiskFile),
}

impl RecordFile for HybridFile {
    fn id(&self) -> FileId {
        match self {
            HybridFile::Mem(f) => f.id(),
            HybridFile::Disk(f) => f.id(),
        }
    }

    fn len(&self) -> usize {
        match self {
            HybridFile::Mem(f) => f.len(),
            HybridFile::Disk(f) => f.len(),
        }
    }

    fn start_ts(&self) -> Option<u64> {
        match self {
            HybridFile::Mem(f) => f.start_ts(),
            HybridFile::Disk(f) => f.start_ts(),
        }
    }

    fn end_ts(&self) -> Option<u64> {
        match self {
            HybridFile::Mem(f) => f.end_ts(),
            HybridFile::Disk(f) => f.end_ts(),
        }
    }

    fn set_start_ts(&mut self, ts: u64) {
        match self {
            HybridFile::Mem(f) => f.set_start_ts(ts),
            HybridFile::Disk(f) => f.set_start_ts(ts),
        }
    }

    fn set_end_ts(&mut self, ts: u64) {
        match self {
            HybridFile::Mem(f) => f.set_end_ts(ts),
            HybridFile::Disk(f) => f.set_end_ts(ts),
        }
    }
}

pub struct HybridStorage {
    mem: MemoryStorage,
    disk: DiskStorage,
}

impl HybridStorage {
    pub fn new(path: &str, max_memory_size: usize) -> Self {
        Self {
            mem: MemoryStorage::new(max_memory_size),
            disk: DiskStorage::new(path),
        }
    }
}

impl Storage<HybridFile> for HybridStorage {
    async fn push(&mut self, file: HybridFile) {
        let file_id = file.id();
        match file {
            HybridFile::Mem(file) => {
                if self.mem.can_push(file.len()) {
                    log::warn!("[HybridStorage] push {:?} to memory", file_id);
                    self.mem.push(file).await;
                } else {
                    if self.disk.can_push(file.len()) {
                        log::warn!("[HybridStorage] memory storage full => fallback to disk with file {:?}", file_id);
                        match self.disk.copy_from_mem(file).await {
                            Ok(file) => {
                                log::warn!("[HybridStorage] pushing {:?} to disk", file_id);
                                self.disk.push(file).await;
                                log::warn!("[HybridStorage] pushed {:?} to disk", file_id);
                            }
                            Err(err) => {
                                log::error!("[HybridStorage] memory storage full but fallback {:?} to disk error {:?}", file_id, err);
                            }
                        }
                    } else {
                        log::warn!("[HybridStorage] memory storage and disk full, {:?} reject", file_id);
                    }
                }
            }
            HybridFile::Disk(file) => {
                if self.disk.can_push(file.len()) {
                    log::warn!("[HybridStorage] pushing {:?} to disk", file_id);
                    self.disk.push(file).await;
                    log::warn!("[HybridStorage] pushed {:?} to disk", file_id);
                } else {
                    log::warn!("[HybridStorage] disk full cannot push {:?} to disk", file_id);
                }
            }
        }
    }

    async fn pop(&mut self, file_id: FileId) -> Option<HybridFile> {
        if let Some(file) = self.mem.pop(file_id).await {
            return Some(HybridFile::Mem(file));
        }

        self.disk.pop(file_id).await.map(HybridFile::Disk)
    }

    fn can_push(&self, len: usize) -> bool {
        self.mem.can_push(len) || self.disk.can_push(len)
    }
}

impl AsyncRead for HybridFile {
    fn poll_read(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            HybridFile::Mem(f) => std::pin::Pin::new(f).poll_read(cx, buf),
            HybridFile::Disk(f) => std::pin::Pin::new(f).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for HybridFile {
    fn poll_write(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            HybridFile::Mem(f) => std::pin::Pin::new(f).poll_write(cx, buf),
            HybridFile::Disk(f) => std::pin::Pin::new(f).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            HybridFile::Mem(f) => std::pin::Pin::new(f).poll_flush(cx),
            HybridFile::Disk(f) => std::pin::Pin::new(f).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            HybridFile::Mem(f) => std::pin::Pin::new(f).poll_shutdown(cx),
            HybridFile::Disk(f) => std::pin::Pin::new(f).poll_shutdown(cx),
        }
    }
}
