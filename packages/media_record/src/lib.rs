use std::{collections::VecDeque, time::Instant};

use media_server_protocol::{
    protobuf::cluster_connector::{RecordReq, RecordRes},
    record::SessionRecordEvent,
};

pub struct MediaRecordStats {
    pending_chunks: usize,
    pending_bytes: usize,
    uploading_chunks: usize,
    uploading_bytes: usize,
    uploaded_chunks: usize,
    uploaded_bytes: usize,
}

pub enum Input {
    Event(u64, Instant, SessionRecordEvent),
    UploadResponse(u64, RecordRes),
}

pub enum Output {
    Stats(MediaRecordStats),
    UploadRequest(u64, RecordReq),
}

pub struct MediaRecordService {
    queue: VecDeque<Output>,
}

impl MediaRecordService {
    pub fn new() -> Self {
        Self { queue: VecDeque::new() }
    }

    pub fn on_tick(&mut self, now: Instant) {}

    pub fn on_input(&mut self, now: Instant, event: Input) {
        match event {
            Input::Event(session, ts, event) => self.on_record(session, ts, event),
            Input::UploadResponse(_, _) => todo!(),
        }
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output> {
        self.queue.pop_front()
    }
}

impl MediaRecordService {
    fn on_record(&mut self, session: u64, ts: Instant, event: SessionRecordEvent) {}
}
