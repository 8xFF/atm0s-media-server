use std::{rc::Rc, sync::RwLock};

use media_server_utils::now_ms;

use super::events::HookEvent;

#[derive(Clone)]
pub struct HookJobData {
    pub payload: HookEvent,
    pub ts: u64,
    on_done: Rc<dyn Fn(String)>,
}

impl HookJobData {
    pub fn ack(&self) {
        (self.on_done)(self.payload.id().to_string());
    }
}

pub trait HookStorage {
    fn push_back(&self, data: HookEvent);
    fn jobs(&self, limit: i16) -> Vec<HookJobData>;
    fn clean_timeout_event(&self, now: u64);
}

#[derive(Default)]
pub struct InMemoryHookStorage {
    queue: Rc<RwLock<Vec<HookJobData>>>,
}

impl InMemoryHookStorage {
    pub fn len(&self) -> usize {
        self.queue.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.read().unwrap().is_empty()
    }
}

impl HookStorage for InMemoryHookStorage {
    fn push_back(&self, data: HookEvent) {
        let cloned_queue = self.queue.clone();
        let ack = move |uuid: String| {
            let mut queue = cloned_queue.write().unwrap();
            queue.retain(|job| job.payload.id() != uuid.as_str());
        };
        let ack = Rc::new(ack);
        let mut queue = self.queue.write().unwrap();
        queue.push(HookJobData {
            payload: data,
            ts: now_ms(),
            on_done: ack,
        });
    }

    fn jobs(&self, limit: i16) -> Vec<HookJobData> {
        let queue = self.queue.read().unwrap();
        let mut jobs = Vec::new();
        for job in queue.iter() {
            jobs.push(job.clone());
            if jobs.len() as i16 >= limit {
                break;
            }
        }
        jobs
    }

    fn clean_timeout_event(&self, now: u64) {
        let mut queue = self.queue.write().unwrap();
        queue.retain(|job| now - job.ts < 5000);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_in_memory_hook_storage() {
        let storage = InMemoryHookStorage::default();
        // let cloned = storage.clone();

        for i in 0..10 {
            let event = HookEvent::Peer {
                uuid: i.to_string(),
                node: 1,
                ts: i,
                session: 1,
                room: "a".to_string(),
                peer: "a".to_string(),
                event: crate::hooks::events::PeerEvent::Joined,
            };
            storage.push_back(event);
        }

        let jobs = storage.jobs(2);
        let job_ids = jobs.iter().map(|job| job.payload.id()).collect::<Vec<&str>>();
        assert_eq!(job_ids, vec!["0", "1"]);

        let first_job = jobs.first().unwrap();
        first_job.ack();
        assert_eq!(storage.len(), 9);

        let jobs = storage.jobs(2);
        let job_ids = jobs.iter().map(|job| job.payload.id()).collect::<Vec<&str>>();
        assert_eq!(job_ids, vec!["1", "2"]);

        storage.clean_timeout_event(now_ms());
        assert_eq!(storage.len(), 9);

        storage.clean_timeout_event(now_ms() + 5000);
        assert_eq!(storage.len(), 0);
    }
}
