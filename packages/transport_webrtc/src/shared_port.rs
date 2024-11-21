use indexmap::IndexMap;
use std::{fmt::Debug, hash::Hash, net::SocketAddr};
use str0m::ice::StunMessage;

#[derive(Debug)]
pub struct SharedUdpPort<Task> {
    task_remotes: IndexMap<SocketAddr, Task>,
    task_remotes_map: IndexMap<Task, Vec<SocketAddr>>,
    task_ufrags: IndexMap<String, Task>,
    task_ufrags_reverse: IndexMap<Task, String>,
}

impl<Task> Default for SharedUdpPort<Task> {
    fn default() -> Self {
        Self {
            task_remotes: IndexMap::new(),
            task_remotes_map: IndexMap::new(),
            task_ufrags: IndexMap::new(),
            task_ufrags_reverse: IndexMap::new(),
        }
    }
}

impl<Task: Debug + Clone + Copy + Hash + PartialEq + Eq> SharedUdpPort<Task> {
    pub fn add_ufrag(&mut self, ufrag: String, task: Task) {
        log::info!("Add ufrag {} to task {:?}", ufrag, task);
        self.task_ufrags.insert(ufrag.clone(), task);
        self.task_ufrags_reverse.insert(task, ufrag);
    }

    pub fn remove_task(&mut self, task: Task) -> Option<()> {
        let ufrag = self.task_ufrags_reverse.swap_remove(&task)?;
        log::info!("Remove task {:?} => ufrag {}", task, ufrag);
        self.task_ufrags.swap_remove(&ufrag)?;
        let remotes = self.task_remotes_map.swap_remove(&task)?;
        for remote in remotes {
            log::info!("     Remove remote {:?} => task {:?}", remote, task);
            self.task_remotes.swap_remove(&remote);
        }
        Some(())
    }

    pub fn map_remote(&mut self, remote: SocketAddr, buf: &[u8]) -> Option<Task> {
        if let Some(task) = self.task_remotes.get(&remote) {
            return Some(*task);
        }

        let msg = StunMessage::parse(buf).ok()?;
        let (stun_username, _other) = msg.split_username()?;
        log::warn!("Received a stun packet from an unknown remote: {:?}, username {}", remote, stun_username);
        let task = self.task_ufrags.get(stun_username)?;
        log::info!("Mapping remote {:?} to task {:?}", remote, task);
        self.task_remotes.insert(remote, *task);
        self.task_remotes_map.entry(*task).or_default().push(remote);
        Some(*task)
    }
}

#[cfg(test)]
mod tests {
    //TODO test correct mapping
    //TODO test invalid request
}
