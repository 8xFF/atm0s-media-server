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

    pub fn start_child_worker(&self, index: usize) {
        let queue = self.job_queue.clone();
        let storage = self.storage.clone();
        tokio::spawn(async move {
            log::debug!("[MediaRecordWorker] start child worker {index}");
            let client = reqwest::Client::new();
            loop {
                log::debug!("[MediaRecordWorker] worker {index} try to unlock queue");
                if let Some((file_id, link)) = {
                    let mut queue_m = queue.lock().await;
                    let front = queue_m.pop_front();
                    drop(queue_m);
                    front
                } {
                    log::info!("[MediaRecordWorker] child worker {index} received upload job for file {:?}", file_id);
                    if let Some(file) = storage.lock().await.pop(file_id).await {
                        match client
                            .put(&link)
                            .header("Content-Length", file.len())
                            .body(reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file)))
                            .send()
                            .await
                        {
                            Ok(res) => {
                                log::info!("[MediaRecordWorker] worker {index} upload {:?} success {}", file_id, res.status());
                            }
                            Err(err) => {
                                log::error!("[MediaRecordWorker] worker {index} upload {:?} error {:?}", file_id, err);
                                // TODO: retry this file in case upload failed
                                // queue.lock().await.push_back((file_id, link));
                                // storage.lock().await.push(file).await;
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    } else {
                        log::error!("[MediaRecordWorker] worker {index} missing file {:?}", file_id);
                    }
                } else {
                    log::debug!("[MediaRecordWorker] child worker {index} dont have job, sleep then retry");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        });
    }

    pub async fn recv(&mut self) -> Result<(), &'static str> {
        let input = self.rx.recv().await.ok_or("channel closed")?;
        match input {
            Input::RecordChunk(file) => {
                log::info!("[MediaRecordWorker] received chunk {:?} size {}", file.id(), file.len());
                let mut storage = self.storage.lock().await;
                if storage.can_push(file.len()) {
                    storage.push(HybridFile::Mem(file)).await;
                } else {
                    log::error!("Upload storage full => reject record file {:?}", file.id());
                }
                Ok(())
            }
            Input::UploadLink(file, link) => {
                log::info!("[MediaRecordWorker] received upload link for file {:?}", file);
                self.job_queue.lock().await.push_back((file, link));
                Ok(())
            }
        }
    }
}
