use std::fmt::{Display, Formatter};

pub struct ClusterSessionUuid {
    node_id: u16,
    ts: u32,
    seq: u16,
}

impl ClusterSessionUuid {
    pub fn new(node_id: u16, ts: u32, seq: u16) -> Self {
        Self { node_id, ts, seq }
    }

    pub fn to_u64(&self) -> u64 {
        let mut v = 0u64;
        v |= (self.node_id as u64) << 48;
        v |= (self.ts as u64) << 16;
        v |= self.seq as u64;
        v
    }

    pub fn from_u64(v: u64) -> Self {
        Self {
            node_id: ((v >> 48) & 0xffff) as u16,
            ts: ((v >> 16) & 0xffffffff) as u32,
            seq: (v & 0xffff) as u16,
        }
    }
}

impl Display for ClusterSessionUuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.node_id, self.ts, self.seq)
    }
}
