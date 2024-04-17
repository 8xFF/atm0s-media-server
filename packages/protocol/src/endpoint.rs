use std::{fmt::Display, str::FromStr};

#[derive(Clone, Copy)]
pub struct ClusterConnId {
    pub node: u32,
    pub node_session: u64,
    pub server_conn: ServerConnId,
}

impl FromStr for ClusterConnId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl Display for ClusterConnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.node, self.node_session, self.server_conn)
    }
}

#[derive(Clone, Copy)]
pub struct ServerConnId {
    pub worker: u16,
    pub index: usize,
}

impl Display for ServerConnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{})", self.worker, self.index)
    }
}
