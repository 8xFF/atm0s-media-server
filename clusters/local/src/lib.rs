use cluster::{ClusterRoom, ClusterRoomIncomingEvent, ClusterRoomError, ClusterRoomOutgoingEvent};

pub struct RoomLocal {

}

#[async_trait::async_trait]
impl ClusterRoom for RoomLocal {
    fn on_event(&mut self, event: ClusterRoomOutgoingEvent) -> Result<(), ClusterRoomError> {
        Ok(())
    }

    async fn recv(&mut self) -> Result<ClusterRoomIncomingEvent, ClusterRoomError> {
        todo!()
    }
}


pub struct ServerLocal {

}

impl ServerLocal {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(&mut self) -> RoomLocal {
        RoomLocal {}
    }
}