use std::io::Write;

use media_server_protocol::record::{SessionRecordHeader, SessionRecordRow};

use crate::storage::{memory::MemoryFile, RecordFile};

pub struct RecordChunkWriter {
    buf: [u8; 1500],
    file: MemoryFile,
}

impl RecordChunkWriter {
    pub fn new(room: &str, peer: &str, session: u64, start_ts: u64, end_ts: u64) -> Self {
        let mut buf = [0; 1500];
        let header = SessionRecordHeader {
            room: room.to_owned(),
            peer: peer.to_owned(),
            session,
            start_ts,
            end_ts,
        };

        let mut file = MemoryFile::default();
        file.set_start_ts(start_ts);
        file.set_end_ts(end_ts);
        let len = header.write_to(&mut buf[4..]).expect("should read");
        buf[0..4].copy_from_slice(&(len as u32).to_be_bytes());
        file.write(&buf[0..len + 4]).expect("should write");

        Self { file, buf }
    }

    pub fn push(&mut self, row: SessionRecordRow) {
        let len = row.write_to(&mut self.buf[4..]).expect("should read");
        self.buf[0..4].copy_from_slice(&(len as u32).to_be_bytes());
        self.file.write(&self.buf[0..len + 4]).expect("should write");
    }

    pub fn take(self) -> MemoryFile {
        self.file
    }
}
