use media_server_protocol::{
    endpoint::{PeerId, RoomId},
    protobuf::cluster_connector::RecordReq,
    record::{SessionRecordEvent, SessionRecordRow},
};

use crate::{raw_record::RecordChunkWriter, storage::memory::MemoryFile};

const MAX_FILE_LEN_MS: u64 = 60_000;

struct RoomState {
    index: u32,
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
                log::info!("[SessionRecord] {} join room {}/{}", self.session, room, peer);
                self.state = Some(RoomState {
                    index: 0,
                    room: room.clone(),
                    peer: peer.clone(),
                    queue: Some(vec![SessionRecordRow { ts: event_ts, event }]),
                });
                None
            }
            SessionRecordEvent::LeaveRoom => {
                log::info!("[SessionRecord] {} leave room", self.session);
                self.pop_file(now_ms, true)
            }
            SessionRecordEvent::Disconnected => {
                self.closed = true;
                self.pop_file(now_ms, true)
            }
            _ => {
                let state = self.state.as_mut()?;
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
        let mut chunk = RecordChunkWriter::new(&state.room, &state.peer, self.session, from_ts, to_ts);

        for row in queue {
            chunk.push(row);
        }
        let index = state.index;
        state.index += 1;
        Some((
            RecordReq {
                room: state.room.clone().into(),
                peer: state.peer.clone().into(),
                session: self.session,
                index,
                from_ts,
                to_ts,
            },
            chunk.take(),
        ))
    }
}

//TODO: test with session record for ensuring split 1 minute chunks
