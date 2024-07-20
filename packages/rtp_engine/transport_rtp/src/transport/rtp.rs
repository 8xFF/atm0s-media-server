use std::{collections::VecDeque, net::IpAddr, time::Instant};

use media_server_protocol::endpoint::{PeerId, RoomId};

use super::{InternalNetInput, InternalOutput, TransportRtpInternal};

pub struct RtpInternal {
    remote: IpAddr,
    room: RoomId,
    peer: PeerId,
    queue: VecDeque<InternalOutput>,
}

impl RtpInternal {
    pub fn new(remote: IpAddr, room: RoomId, peer: PeerId) -> Self {
        Self {
            remote,
            room,
            peer,
            queue: VecDeque::new(),
        }
    }
}

impl RtpInternal {}

impl TransportRtpInternal for RtpInternal {
    fn on_tick(&mut self, now: Instant) {}

    fn handle_input(&mut self, input: InternalNetInput) {}

    fn pop_output(&mut self, now: std::time::Instant) -> Option<super::InternalOutput> {
        self.queue.pop_front()
    }
}
