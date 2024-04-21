use std::{fmt::Display, str::FromStr};

use crate::{
    media::{MediaCodec, MediaKind, MediaScaling},
    transport::ConnLayer,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClusterConnId {
    pub node: u32,
    pub node_session: u64,
    pub server_conn: ServerConnId,
}

impl FromStr for ClusterConnId {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split('-').collect::<Vec<_>>();
        let node = parts.get(0).ok_or("MISSING NODE_ID")?.parse::<u32>().map_err(|_| "PARSE ERROR NODE_ID")?;
        let node_session = parts.get(1).ok_or("MISSING NODE_SESSION")?.parse::<u64>().map_err(|_| "PARSE ERROR NODE_SESSION")?;
        let server_conn = parts.get(2).ok_or("MISSING SERVER_CONN")?.parse::<ServerConnId>().map_err(|_| "PARSE ERROR SERVER_CONN")?;
        Ok(Self { node, node_session, server_conn })
    }
}

impl Display for ClusterConnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.node, self.node_session, self.server_conn)
    }
}

impl ConnLayer for ClusterConnId {
    type Up = ();
    type UpParam = ();
    type Down = ServerConnId;
    type DownRes = (u32, u64);

    fn down(self) -> (Self::Down, Self::DownRes) {
        (self.server_conn, (self.node, self.node_session))
    }

    fn up(self, _param: Self::UpParam) -> Self::Up {
        panic!("should not happen")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServerConnId {
    pub worker: u16,
    pub index: usize,
}

impl FromStr for ServerConnId {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(',').collect::<Vec<_>>();
        let worker = parts.get(0).ok_or("MISSING WORKER")?.parse::<u16>().map_err(|_| "PARSE ERROR WORKER")?;
        let index = parts.get(1).ok_or("MISSING INDEX")?.parse::<usize>().map_err(|_| "PARSE ERROR INDEX")?;
        Ok(Self { worker, index })
    }
}

impl Display for ServerConnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.worker, self.index)
    }
}

impl ConnLayer for ServerConnId {
    type Up = ClusterConnId;
    type UpParam = (u32, u64);
    type Down = usize;
    type DownRes = u16;

    fn down(self) -> (Self::Down, Self::DownRes) {
        (self.index, self.worker)
    }

    fn up(self, param: Self::UpParam) -> Self::Up {
        ClusterConnId {
            node: param.0,
            node_session: param.1,
            server_conn: self,
        }
    }
}

impl ConnLayer for usize {
    type Up = ServerConnId;
    type UpParam = u16;
    type Down = ();
    type DownRes = ();

    fn down(self) -> (Self::Down, Self::DownRes) {
        panic!("should not happen")
    }

    fn up(self, param: Self::UpParam) -> Self::Up {
        ServerConnId { index: self, worker: param }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RoomId(pub String);

#[derive(Clone, PartialEq, Eq)]
pub struct PeerId(pub String);

#[derive(Clone, PartialEq, Eq)]
pub struct TrackName(pub String);

#[derive(Clone)]
pub struct TrackMeta {
    pub kind: MediaKind,
    pub codec: MediaCodec,
    pub scaling: MediaScaling,
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::{ClusterConnId, ServerConnId};

    #[test]
    fn server_conn_id_parse() {
        let conn = ServerConnId { worker: 1, index: 2 };
        assert_eq!(conn.to_string(), "1,2");
        assert_eq!(ServerConnId::from_str("1,2"), Ok(ServerConnId { worker: 1, index: 2 }));
    }

    #[test]
    fn cluster_conn_id_pase() {
        let conn = ClusterConnId {
            node: 1,
            node_session: 2,
            server_conn: ServerConnId { worker: 3, index: 4 },
        };
        assert_eq!(conn.to_string(), "1-2-3,4");
        assert_eq!(
            ClusterConnId::from_str("1-2-3,4"),
            Ok(ClusterConnId {
                node: 1,
                node_session: 2,
                server_conn: ServerConnId { worker: 3, index: 4 },
            })
        );
    }
}
