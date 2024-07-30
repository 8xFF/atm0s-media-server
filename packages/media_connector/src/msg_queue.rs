use std::collections::{BTreeMap, HashMap, VecDeque};

const RESEND_AFTER_MS: u64 = 1000;

pub trait Message {
    fn msg_id(&self) -> u64;
}

#[derive(Default)]
pub struct MessageQueue<M, const MAX_INFLIGHT: usize> {
    inflight_ts: BTreeMap<u64, Vec<u64>>,
    inflight: HashMap<u64, M>,
    queue: VecDeque<M>,
    acked: usize,
}

impl<M: Message, const MAX_INFLIGHT: usize> MessageQueue<M, MAX_INFLIGHT> {
    pub fn push(&mut self, msg: M) {
        self.queue.push_back(msg);
    }

    pub fn on_ack(&mut self, id: u64) {
        if self.inflight.remove(&id).is_some() {
            self.acked += 1;
            log::debug!("[ConnectorAgent/MessageQueue] msg for ack {id}");
        } else {
            log::warn!("[ConnectorAgent/MessageQueue] msg for ack {id} not found");
        }
    }

    pub fn pop(&mut self, now_ms: u64) -> Option<&M> {
        if let Some(msg_id) = self.pop_retry_msg_id(now_ms) {
            let entry = self.inflight_ts.entry(now_ms).or_default();
            entry.push(msg_id);
            return Some(self.inflight.get(&msg_id).expect("should exist retry_msg_id"));
        }

        if self.inflight.len() < MAX_INFLIGHT {
            let msg = self.queue.pop_front()?;
            let msg_id = msg.msg_id();
            let entry = self.inflight_ts.entry(now_ms).or_default();
            entry.push(msg_id);
            self.inflight.insert(msg_id, msg);
            self.inflight.get(&msg_id)
        } else {
            None
        }
    }

    pub fn waits(&self) -> usize {
        self.queue.len()
    }

    pub fn inflight(&self) -> usize {
        self.inflight.len()
    }

    pub fn acked(&self) -> usize {
        self.acked
    }

    fn pop_retry_msg_id(&mut self, now_ms: u64) -> Option<u64> {
        loop {
            let mut entry = self.inflight_ts.first_entry()?;
            if *entry.key() + RESEND_AFTER_MS <= now_ms {
                let msg_id = entry.get_mut().pop().expect("should have msg");
                if entry.get().is_empty() {
                    entry.remove();
                }
                if self.inflight.contains_key(&msg_id) {
                    break Some(msg_id);
                }
            } else {
                break None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::msg_queue::RESEND_AFTER_MS;

    use super::{Message, MessageQueue};

    impl Message for u64 {
        fn msg_id(&self) -> u64 {
            *self
        }
    }

    #[test]
    fn simple_work() {
        let mut queue = MessageQueue::<u64, 1>::default();
        queue.push(1);
        queue.push(2);

        assert_eq!(queue.pop(0), Some(&1));
        assert_eq!(queue.pop(0), None);

        queue.on_ack(1);
        assert_eq!(queue.pop(0), Some(&2));
        assert_eq!(queue.pop(0), None);

        queue.on_ack(2);
        assert_eq!(queue.pop(0), None);
        assert_eq!(queue.inflight.len(), 0);
        assert_eq!(queue.inflight_ts.len(), 1);

        assert_eq!(queue.pop(RESEND_AFTER_MS), None);
        assert_eq!(queue.inflight_ts.len(), 0);
    }

    #[test]
    fn retry_msg() {
        let mut queue = MessageQueue::<u64, 1>::default();
        queue.push(1);
        assert_eq!(queue.pop(0), Some(&1));
        assert_eq!(queue.pop(0), None);

        assert_eq!(queue.pop(RESEND_AFTER_MS), Some(&1));
        assert_eq!(queue.pop(RESEND_AFTER_MS), None);
    }
}
