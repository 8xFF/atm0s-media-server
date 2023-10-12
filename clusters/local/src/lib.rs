use cluster::{Cluster, ClusterRoom, ClusterRoomError, ClusterRoomIncomingEvent, ClusterRoomOutgoingEvent};

pub struct RoomLocal {}

#[async_trait::async_trait]
impl ClusterRoom for RoomLocal {
    fn on_event(&mut self, event: ClusterRoomOutgoingEvent) -> Result<(), ClusterRoomError> {
        Ok(())
    }

    async fn recv(&mut self) -> Result<ClusterRoomIncomingEvent, ClusterRoomError> {
        async_std::task::sleep(std::time::Duration::from_secs(1000)).await;
        todo!("implement recv")
    }
}

pub struct ServerLocal {}

impl ServerLocal {
    pub fn new() -> Self {
        Self {}
    }
}

impl Cluster<RoomLocal> for ServerLocal {
    fn build(&mut self) -> RoomLocal {
        RoomLocal {}
    }
}
