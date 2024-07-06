use std::collections::{HashMap, VecDeque};

use media_server_protocol::{
    protobuf::cluster_connector::{RecordReq, RecordRes},
    record::SessionRecordEvent,
};
use session::SessionRecord;
use storage::{memory::MemoryFile, FileId, RecordFile};
use tokio::sync::mpsc::Sender;
use worker::UploadWorker;

mod session;
mod storage;
mod worker;

pub struct MediaRecordStats {
    pending_chunks: usize,
    pending_bytes: usize,
    uploading_chunks: usize,
    uploading_bytes: usize,
    uploaded_chunks: usize,
    uploaded_bytes: usize,
}

pub enum Input {
    Event(u64, u64, SessionRecordEvent),
    UploadResponse(u64, RecordRes),
}

pub enum Output {
    Stats(MediaRecordStats),
    UploadRequest(u64, RecordReq),
}

pub struct MediaRecordService {
    req_id_seed: u64,
    queue: VecDeque<Output>,
    chunk_map: HashMap<u64, FileId>,
    sessions: HashMap<u64, SessionRecord>,
    worker_tx: Sender<worker::Input>,
}

impl MediaRecordService {
    pub fn new(workers: usize, path: &str, max_mem_size: usize) -> Self {
        let (mut worker, worker_tx) = UploadWorker::new(path, max_mem_size);
        for _ in 0..workers {
            worker.start_child_worker();
        }

        tokio::spawn(async move {
            loop {
                if let Err(e) = worker.recv().await {
                    log::error!("worker error {e}");
                }
            }
        });

        Self {
            req_id_seed: 0,
            queue: VecDeque::new(),
            sessions: HashMap::new(),
            chunk_map: HashMap::new(),
            worker_tx,
        }
    }

    pub fn on_tick(&mut self, now: u64) {
        for (_id, session) in self.sessions.iter_mut() {
            if let Some((req, file)) = session.tick(now) {
                Self::process_chunk(&mut self.req_id_seed, req, file, &mut self.queue, &mut self.chunk_map, &self.worker_tx);
            }
        }
        self.sessions.retain(|_, session| !session.is_closed());
    }

    pub fn on_input(&mut self, now: u64, event: Input) {
        match event {
            Input::Event(session, ts, event) => self.on_record_event(now, session, ts, event),
            Input::UploadResponse(req_id, res) => {
                if let Some(file_id) = self.chunk_map.remove(&req_id) {
                    if let Err(e) = self.worker_tx.try_send(worker::Input::UploadLink(file_id, res.s3_uri)) {
                        log::error!("[MediaWorkerService] send record link to record controller worker error {e}");
                    }
                }
            }
        }
    }

    pub fn pop_output(&mut self) -> Option<Output> {
        self.queue.pop_front()
    }
}

impl MediaRecordService {
    fn on_record_event(&mut self, now_ms: u64, session: u64, event_ts: u64, event: SessionRecordEvent) {
        let session = self.sessions.entry(session).or_insert_with(|| SessionRecord::new(session));
        if let Some((req, file)) = session.push(now_ms, event_ts, event) {
            Self::process_chunk(&mut self.req_id_seed, req, file, &mut self.queue, &mut self.chunk_map, &self.worker_tx);
        }
    }

    fn process_chunk(req_seed: &mut u64, req: RecordReq, file: MemoryFile, queue: &mut VecDeque<Output>, chunk_map: &mut HashMap<u64, FileId>, worker_tx: &Sender<worker::Input>) {
        let req_id = *req_seed;
        *req_seed += 1;
        queue.push_back(Output::UploadRequest(req_id, req));
        chunk_map.insert(req_id, file.id());
        if let Err(e) = worker_tx.try_send(worker::Input::RecordChunk(file)) {
            log::error!("[MediaWorkerService] send record chunk to record controller worker error {e}");
        }
    }
}
