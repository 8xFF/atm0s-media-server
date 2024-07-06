use std::io::{Read, Write};

use media_server_protocol::{
    endpoint::{PeerId, RoomId},
    protobuf::cluster_connector::RecordReq,
    record::{SessionRecordEvent, SessionRecordRow},
};

use crate::storage::{memory::MemoryFile, RecordFile};

const MAX_FILE_LEN_MS: u64 = 60_000;

struct RoomState {
    room: RoomId,
    peer: PeerId,
    queue: Option<Vec<SessionRecordRow>>,
}

pub struct SessionRecord {
    session: u64,
    state: Option<RoomState>,
    closed: bool,
}

impl SessionRecord {
    pub fn new(session: u64) -> Self {
        Self { session, state: None, closed: false }
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn tick(&mut self, now_ms: u64) -> Option<(RecordReq, MemoryFile)> {
        self.pop_file(now_ms, false)
    }

    pub fn push(&mut self, now_ms: u64, event_ts: u64, event: SessionRecordEvent) -> Option<(RecordReq, MemoryFile)> {
        if self.closed {
            return self.pop_file(now_ms, true);
        }

        match &event {
            SessionRecordEvent::JoinRoom(room, peer) => {
                self.state = Some(RoomState {
                    room: room.clone(),
                    peer: peer.clone(),
                    queue: Some(vec![SessionRecordRow { ts: event_ts, event }]),
                });
                None
            }
            SessionRecordEvent::LeaveRoom => self.pop_file(now_ms, true),
            SessionRecordEvent::Disconnected => {
                self.closed = true;
                self.pop_file(now_ms, true)
            }
            _ => {
                let state = self.state.as_mut().expect("Should have state");
                if let Some(queue) = &mut state.queue {
                    queue.push(SessionRecordRow { ts: event_ts, event });
                } else {
                    state.queue = Some(vec![SessionRecordRow { ts: event_ts, event }]);
                }
                self.pop_file(now_ms, false)
            }
        }
    }

    fn pop_file(&mut self, now_ms: u64, force: bool) -> Option<(RecordReq, MemoryFile)> {
        let state = self.state.as_mut()?;
        let queue = state.queue.as_mut()?;
        let from_ts = queue.first()?.ts;
        let to_ts = queue.last()?.ts;
        let duration_ms = to_ts - from_ts;
        let ttl_ms = now_ms - to_ts;
        let should_pop = force || duration_ms >= MAX_FILE_LEN_MS || ttl_ms >= MAX_FILE_LEN_MS;
        if !should_pop {
            return None;
        }

        let queue = state.queue.take()?;
        let mut file = MemoryFile::default();
        file.set_start_ts(from_ts);
        file.set_end_ts(to_ts);
        let mut buf = [0; 1500];
        for mut row in queue {
            let len = row.read(&mut buf[4..]).expect("should read");
            buf[0..4].copy_from_slice(&(len as u32).to_be_bytes());
            file.write(&buf[0..len]).expect("should write");
        }
        Some((
            RecordReq {
                room: state.room.0.clone(),
                peer: state.peer.0.clone(),
                session: self.session,
                from_ts,
                to_ts,
            },
            file,
        ))
    }
}
