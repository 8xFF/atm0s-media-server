use std::{collections::VecDeque, sync::Arc, time::Duration};

use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    Mutex,
};

use crate::storage::{memory::MemoryFile, FileId, HybridFile, HybridStorage, RecordFile, Storage};

pub enum Input {
    RecordChunk(MemoryFile),
    UploadLink(FileId, String),
}

pub struct UploadWorker {
    storage: Arc<Mutex<HybridStorage>>,
    job_queue: Arc<Mutex<VecDeque<(FileId, String)>>>,
    rx: Receiver<Input>,
}

impl UploadWorker {
    pub fn new(path: &str, max_memory_size: usize) -> (Self, Sender<Input>) {
        let (tx, rx) = channel(10);
        let job_queue: Arc<Mutex<VecDeque<(FileId, String)>>> = Default::default();
        (
            Self {
                storage: Arc::new(Mutex::new(HybridStorage::new(path, max_memory_size))),
                rx,
                job_queue,
            },
            tx,
        )
    }

    pub fn start_child_worker(&self) {
        let queue = self.job_queue.clone();
        let storage = self.storage.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            loop {
                if let Some((file_id, link)) = queue.lock().await.pop_front() {
                    if let Some(file) = storage.lock().await.pop(file_id).await {
                        let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file));
                        match client.put(&link).body(body).send().await {
                            Ok(res) => {
                                log::info!("[MediaRecordWorker] upload {:?} success {}", file_id, res.status());
                            }
                            Err(err) => {
                                log::error!("[MediaRecordWorker] upload {:?} error {:?}", file_id, err);
                                // TODO retry this file
                                // queue.lock().await.push_back((file_id, link));
                                // storage.lock().await.push(file).await;
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    } else {
                        log::error!("[MediaRecordWorker] missing file {:?}", file_id);
                    }
                } else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }

    pub async fn recv(&mut self) -> Result<(), &'static str> {
        let input = self.rx.recv().await.ok_or("channel closed")?;
        match input {
            Input::RecordChunk(file) => {
                let mut storage = self.storage.lock().await;
                if storage.can_push(file.len()) {
                    storage.push(HybridFile::Mem(file)).await;
                } else {
                    log::error!("Upload storage full => reject record file {:?}", file.id());
                }
                Ok(())
            }
            Input::UploadLink(file, link) => {
                self.job_queue.lock().await.push_back((file, link));
                Ok(())
            }
        }
    }
}
